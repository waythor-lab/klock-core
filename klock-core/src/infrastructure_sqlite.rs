//! SQLite-backed LeaseStore implementation.
//! Provides persistent lease storage across server restarts.
//!
//! Enable with the `sqlite` feature flag:
//! ```toml
//! klock-core = { path = "../klock-core", features = ["sqlite"] }
//! ```

use rusqlite::{params, Connection};
use std::collections::HashMap;

use crate::infrastructure::LeaseStore;
use crate::scheduler::{VerdictStatus, WaitDieScheduler};
use crate::types::*;

/// A persistent lease store backed by SQLite.
///
/// Uses WAL mode for concurrent read performance.
pub struct SqliteLeaseStore {
    conn: Connection,
    priorities: HashMap<String, u64>,
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

        Ok(Self { conn, priorities })
    }

    /// Register an agent with a priority timestamp.
    pub fn register_agent_priority(&mut self, agent_id: String, priority: u64) {
        self.conn
            .execute(
                "INSERT OR REPLACE INTO agent_priorities (agent_id, priority) VALUES (?1, ?2)",
                params![agent_id, priority],
            )
            .ok();
        self.priorities.insert(agent_id, priority);
    }

    /// Get the priority map (for scheduler).
    pub fn get_priorities(&self) -> HashMap<String, u64> {
        self.priorities.clone()
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
        // Evict expired first
        self.evict_expired(now);

        let active_leases = self.get_active_leases();

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

                self.conn
                    .execute(
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
                    )
                    .ok();

                LeaseResult::Success { lease }
            }
        }
    }

    fn release(&mut self, lease_id: &str) -> bool {
        let rows = self
            .conn
            .execute(
                "UPDATE leases SET state = 'Released' WHERE id = ?1 AND state = 'Active'",
                params![lease_id],
            )
            .unwrap_or(0);
        rows > 0
    }

    fn heartbeat(&mut self, lease_id: &str, now: u64) -> bool {
        // Get the lease's TTL to calculate new expiry
        let ttl: Option<u64> = self
            .conn
            .query_row(
                "SELECT ttl FROM leases WHERE id = ?1 AND state = 'Active'",
                params![lease_id],
                |row| row.get(0),
            )
            .ok();

        if let Some(ttl) = ttl {
            let new_expires = now + ttl;
            let rows = self
                .conn
                .execute(
                    "UPDATE leases SET last_heartbeat = ?1, expires_at = ?2 WHERE id = ?3 AND state = 'Active'",
                    params![now, new_expires, lease_id],
                )
                .unwrap_or(0);
            rows > 0
        } else {
            false
        }
    }

    fn get_active_leases(&self) -> Vec<Lease> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, agent_id, session_id, res_type, res_path, predicate, state, acquired_at, ttl, expires_at, last_heartbeat
                 FROM leases WHERE state = 'Active'",
            )
            .expect("Failed to prepare statement");

        stmt.query_map([], |row| Self::row_to_lease(row))
            .expect("Failed to query leases")
            .filter_map(|r| r.ok())
            .collect()
    }

    fn evict_expired(&mut self, now: u64) -> usize {
        self.conn
            .execute(
                "UPDATE leases SET state = 'Expired' WHERE state = 'Active' AND expires_at < ?1",
                params![now],
            )
            .unwrap_or(0)
    }
}
