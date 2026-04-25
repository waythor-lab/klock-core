//! SQLite-backed LeaseStore implementation.
//! Provides persistent lease storage across server restarts.
//!
//! Enable with the `sqlite` feature flag:
//! ```toml
//! klock-core = { path = "../klock-core", features = ["sqlite"] }
//! ```
//!
//! ## Failure model
//!
//! When the store hits a non-recoverable error (corrupt schema, dropped
//! table, etc.) it sets a `poisoned` flag and refuses subsequent acquires
//! with `LeaseFailureReason::StorageUnavailable`. The HTTP layer maps this
//! to a 503 so a load balancer can pull the node. Reads return an empty
//! `Vec<Lease>` after poisoning, but acquires no longer rely on those
//! reads to make grant decisions — they short-circuit instead.

use rusqlite::{Connection, params};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::infrastructure::LeaseStore;
use crate::scheduler::{VerdictStatus, WaitDieScheduler};
use crate::types::*;

/// A persistent lease store backed by SQLite.
///
/// Uses WAL mode for concurrent read performance.
pub struct SqliteLeaseStore {
    conn: Connection,
    priorities: HashMap<String, u64>,
    poisoned: AtomicBool,
}

impl SqliteLeaseStore {
    /// Open (or create) a SQLite database at the given path.
    pub fn open(path: &str) -> Result<Self, rusqlite::Error> {
        let conn = Connection::open(path)?;

        // Enable WAL mode for better concurrent read performance
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS leases (
                id          TEXT PRIMARY KEY,
                agent_id    TEXT NOT NULL,
                session_id  TEXT NOT NULL,
                res_type    TEXT NOT NULL,
                res_path    TEXT NOT NULL,
                predicate   TEXT NOT NULL,
                state       TEXT NOT NULL DEFAULT 'Active',
                acquired_at INTEGER NOT NULL,
                ttl         INTEGER NOT NULL,
                expires_at  INTEGER NOT NULL,
                last_heartbeat INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_leases_state ON leases(state);
            CREATE INDEX IF NOT EXISTS idx_leases_resource ON leases(res_type, res_path);

            CREATE TABLE IF NOT EXISTS agent_priorities (
                agent_id TEXT PRIMARY KEY,
                priority INTEGER NOT NULL
            );",
        )?;

        // Load priorities into memory for fast access
        let mut priorities = HashMap::new();
        {
            let mut stmt = conn.prepare("SELECT agent_id, priority FROM agent_priorities")?;
            let rows = stmt.query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, u64>(1)?))
            })?;
            for row in rows {
                let (agent_id, priority) = row?;
                priorities.insert(agent_id, priority);
            }
        }

        Ok(Self {
            conn,
            priorities,
            poisoned: AtomicBool::new(false),
        })
    }

    /// Register an agent with a priority timestamp.
    ///
    /// The in-memory priority map is only updated when the disk write
    /// succeeds. Otherwise priorities silently regress on restart, breaking
    /// Wait-Die guarantees.
    pub fn register_agent_priority(&mut self, agent_id: String, priority: u64) {
        match self.conn.execute(
            "INSERT OR REPLACE INTO agent_priorities (agent_id, priority) VALUES (?1, ?2)",
            params![agent_id, priority],
        ) {
            Ok(_) => {
                self.priorities.insert(agent_id, priority);
            }
            Err(e) => {
                tracing::error!(
                    error = %e,
                    agent_id = %agent_id,
                    "Failed to persist agent priority; in-memory map left unchanged to avoid divergence"
                );
            }
        }
    }

    /// Get the priority map (for scheduler).
    pub fn get_priorities(&self) -> HashMap<String, u64> {
        self.priorities.clone()
    }

    /// True if a previous storage error has poisoned this store.
    /// While poisoned, `acquire` returns `StorageUnavailable` immediately.
    pub fn is_poisoned(&self) -> bool {
        self.poisoned.load(Ordering::Acquire)
    }

    fn poison(&self) {
        self.poisoned.store(true, Ordering::Release);
    }

    fn parse_predicate(s: &str) -> Predicate {
        match s {
            "Provides" => Predicate::Provides,
            "Consumes" => Predicate::Consumes,
            "Mutates" => Predicate::Mutates,
            "Deletes" => Predicate::Deletes,
            "DependsOn" => Predicate::DependsOn,
            "Renames" => Predicate::Renames,
            _ => Predicate::Consumes,
        }
    }

    fn parse_resource_type(s: &str) -> ResourceType {
        match s {
            "File" => ResourceType::File,
            "Symbol" => ResourceType::Symbol,
            "ApiEndpoint" => ResourceType::ApiEndpoint,
            "DatabaseTable" => ResourceType::DatabaseTable,
            "ConfigKey" => ResourceType::ConfigKey,
            _ => ResourceType::File,
        }
    }

    fn parse_lease_state(s: &str) -> LeaseState {
        match s {
            "Active" => LeaseState::Active,
            "Expired" => LeaseState::Expired,
            "Released" => LeaseState::Released,
            "Revoked" => LeaseState::Revoked,
            _ => LeaseState::Active,
        }
    }

    fn row_to_lease(row: &rusqlite::Row) -> rusqlite::Result<Lease> {
        let predicate_str: String = row.get(5)?;
        let res_type_str: String = row.get(3)?;
        let state_str: String = row.get(6)?;

        Ok(Lease {
            id: row.get(0)?,
            agent_id: row.get(1)?,
            session_id: row.get(2)?,
            resource: ResourceRef::new(
                Self::parse_resource_type(&res_type_str),
                row.get::<_, String>(4)?,
            ),
            predicate: Self::parse_predicate(&predicate_str),
            state: Self::parse_lease_state(&state_str),
            acquired_at: row.get(7)?,
            ttl: row.get(8)?,
            expires_at: row.get(9)?,
            last_heartbeat: row.get(10)?,
        })
    }

    fn storage_unavailable() -> LeaseResult {
        LeaseResult::Failure {
            reason: LeaseFailureReason::StorageUnavailable,
            existing_lease: None,
            wait_time: None,
        }
    }
}

impl LeaseStore for SqliteLeaseStore {
    fn acquire(
        &mut self,
        agent_id: &str,
        session_id: &str,
        resource: ResourceRef,
        predicate: Predicate,
        ttl: u64,
        now: u64,
    ) -> LeaseResult {
        // Fail closed if a prior storage error poisoned the store. The
        // scheduler must never grant on a stale or empty lease view.
        if self.is_poisoned() {
            return Self::storage_unavailable();
        }

        // Evict expired first
        self.evict_expired(now);

        let active_leases = self.get_active_leases();

        // The eviction or read could have poisoned us.
        if self.is_poisoned() {
            return Self::storage_unavailable();
        }

        // Check Wait-Die scheduler
        let verdict = WaitDieScheduler::decide(
            agent_id,
            predicate,
            &resource,
            &active_leases,
            &self.priorities,
        );

        match verdict.status {
            VerdictStatus::Wait => LeaseResult::Failure {
                reason: LeaseFailureReason::Wait,
                existing_lease: None,
                wait_time: None,
            },
            VerdictStatus::Die => LeaseResult::Failure {
                reason: LeaseFailureReason::Die,
                existing_lease: None,
                wait_time: verdict.retry_after_ms,
            },
            VerdictStatus::Granted => {
                let lease_id = format!("lease_{}_{}", agent_id, now);
                let lease = Lease::new(
                    lease_id.clone(),
                    agent_id.to_string(),
                    session_id.to_string(),
                    resource.clone(),
                    predicate,
                    ttl,
                    now,
                );

                let insert_result = self.conn.execute(
                    "INSERT INTO leases (id, agent_id, session_id, res_type, res_path, predicate, state, acquired_at, ttl, expires_at, last_heartbeat)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'Active', ?7, ?8, ?9, ?10)",
                    params![
                        lease.id,
                        lease.agent_id,
                        lease.session_id,
                        format!("{:?}", resource.resource_type),
                        resource.path,
                        format!("{:?}", predicate),
                        lease.acquired_at,
                        lease.ttl,
                        lease.expires_at,
                        lease.last_heartbeat,
                    ],
                );

                match insert_result {
                    Ok(1) => LeaseResult::Success { lease },
                    Ok(rows) => {
                        tracing::error!(
                            lease_id = %lease.id,
                            rows_affected = rows,
                            "Lease INSERT returned unexpected row count; poisoning store"
                        );
                        self.poison();
                        Self::storage_unavailable()
                    }
                    Err(e) => {
                        tracing::error!(
                            error = %e,
                            lease_id = %lease.id,
                            "Failed to persist lease; poisoning store"
                        );
                        self.poison();
                        Self::storage_unavailable()
                    }
                }
            }
        }
    }

    fn release(&mut self, lease_id: &str) -> bool {
        match self.conn.execute(
            "UPDATE leases SET state = 'Released' WHERE id = ?1 AND state = 'Active'",
            params![lease_id],
        ) {
            Ok(rows) => rows > 0,
            Err(e) => {
                tracing::error!(error = %e, lease_id = %lease_id, "Failed to release lease");
                false
            }
        }
    }

    fn heartbeat(&mut self, lease_id: &str, now: u64) -> bool {
        // Get the lease's TTL to calculate new expiry
        let ttl: Option<u64> = match self.conn.query_row(
            "SELECT ttl FROM leases WHERE id = ?1 AND state = 'Active'",
            params![lease_id],
            |row| row.get(0),
        ) {
            Ok(v) => Some(v),
            Err(rusqlite::Error::QueryReturnedNoRows) => None,
            Err(e) => {
                tracing::error!(error = %e, lease_id = %lease_id, "Heartbeat lookup failed");
                None
            }
        };

        if let Some(ttl) = ttl {
            let new_expires = now + ttl;
            match self.conn.execute(
                "UPDATE leases SET last_heartbeat = ?1, expires_at = ?2 WHERE id = ?3 AND state = 'Active'",
                params![now, new_expires, lease_id],
            ) {
                Ok(rows) => rows > 0,
                Err(e) => {
                    tracing::error!(error = %e, lease_id = %lease_id, "Heartbeat UPDATE failed");
                    false
                }
            }
        } else {
            false
        }
    }

    fn get_active_leases(&self) -> Vec<Lease> {
        let mut stmt = match self.conn.prepare(
            "SELECT id, agent_id, session_id, res_type, res_path, predicate, state, acquired_at, ttl, expires_at, last_heartbeat
             FROM leases WHERE state = 'Active'",
        ) {
            Ok(s) => s,
            Err(e) => {
                tracing::error!(error = %e, "Failed to prepare get_active_leases statement; poisoning store");
                self.poison();
                return Vec::new();
            }
        };

        let rows = match stmt.query_map([], |row| Self::row_to_lease(row)) {
            Ok(r) => r,
            Err(e) => {
                tracing::error!(error = %e, "Failed to query active leases; poisoning store");
                self.poison();
                return Vec::new();
            }
        };

        // Iterate explicitly so a row-level error (corrupt row, schema
        // mismatch detected lazily, dropped table seen mid-iteration)
        // poisons the store rather than being silently filtered out.
        let mut leases = Vec::new();
        for row in rows {
            match row {
                Ok(l) => leases.push(l),
                Err(e) => {
                    tracing::error!(error = %e, "Row read failed during get_active_leases; poisoning store");
                    self.poison();
                    return Vec::new();
                }
            }
        }
        leases
    }

    fn evict_expired(&mut self, now: u64) -> usize {
        match self.conn.execute(
            "UPDATE leases SET state = 'Expired' WHERE state = 'Active' AND expires_at < ?1",
            params![now],
        ) {
            Ok(rows) => rows,
            Err(e) => {
                tracing::error!(error = %e, "Failed to evict expired leases");
                0
            }
        }
    }
}
