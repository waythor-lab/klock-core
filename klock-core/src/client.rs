//! High-level ergonomic client that wraps the pure kernel + pluggable storage.
//! Both the napi-rs (JS) and PyO3 (Python) FFI layers delegate to this.

use crate::infrastructure::LeaseStore;
use crate::infrastructure_in_memory::InMemoryLeaseStore;
use crate::state::{IntentManifest, KernelVerdict, KlockKernel, StateSnapshot};
use crate::types::*;
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// Trait combining LeaseStore with agent priority management plus
/// a passthrough for the storage-poisoned health flag.
/// Allows KlockClient to be generic over storage backends.
pub trait LeaseStoreExt: LeaseStore {
    fn register_agent_priority(&mut self, agent_id: String, priority: u64);
    fn get_priorities(&self) -> HashMap<String, u64>;
    /// True when the underlying storage has hit a non-recoverable error
    /// and the store is now refusing acquires to fail closed.
    fn is_poisoned(&self) -> bool {
        false
    }
}

impl LeaseStoreExt for InMemoryLeaseStore {
    fn register_agent_priority(&mut self, agent_id: String, priority: u64) {
        InMemoryLeaseStore::register_agent_priority(self, agent_id, priority);
    }
    fn get_priorities(&self) -> HashMap<String, u64> {
        InMemoryLeaseStore::get_priorities(self)
    }
}

#[cfg(feature = "sqlite")]
impl LeaseStoreExt for crate::infrastructure_sqlite::SqliteLeaseStore {
    fn register_agent_priority(&mut self, agent_id: String, priority: u64) {
        crate::infrastructure_sqlite::SqliteLeaseStore::register_agent_priority(
            self, agent_id, priority,
        );
    }
    fn get_priorities(&self) -> HashMap<String, u64> {
        crate::infrastructure_sqlite::SqliteLeaseStore::get_priorities(self)
    }
    fn is_poisoned(&self) -> bool {
        crate::infrastructure_sqlite::SqliteLeaseStore::is_poisoned(self)
    }
}

/// The main entry point for using Klock. Manages agents, leases, and
/// conflict resolution through a single ergonomic API.
///
/// The kernel's view of "active intents" is derived from active leases at
/// snapshot time, so there is no separate intent ledger to keep in sync.
pub struct KlockClient {
    store: Box<dyn LeaseStoreExt + Send>,
    /// Counter for generating unique IDs
    id_counter: u64,
}

impl KlockClient {
    /// Create a new KlockClient with an empty in-memory store.
    pub fn new() -> Self {
        Self {
            store: Box::new(InMemoryLeaseStore::new()),
            id_counter: 0,
        }
    }

    /// Create a new KlockClient backed by SQLite at the given path.
    /// Leases persist across server restarts.
    #[cfg(feature = "sqlite")]
    pub fn with_sqlite(path: &str) -> Result<Self, String> {
        let store = crate::infrastructure_sqlite::SqliteLeaseStore::open(path)
            .map_err(|e| format!("Failed to open SQLite database at '{}': {}", path, e))?;
        Ok(Self {
            store: Box::new(store),
            id_counter: 0,
        })
    }

    /// Register an agent with a priority timestamp.
    /// Lower timestamps = higher priority (older = senior).
    pub fn register_agent(&mut self, agent_id: &str, priority: u64) {
        self.store
            .register_agent_priority(agent_id.to_string(), priority);
    }

    /// Declare an intent manifest and get a kernel verdict.
    /// This checks for conflicts and applies Wait-Die scheduling.
    ///
    /// Intents are advisory: a `Granted` verdict gates a future `acquire_lease`
    /// but does not itself reserve anything. The kernel's "active intents" view
    /// is derived from currently held leases.
    pub fn declare_intent(&mut self, manifest: &IntentManifest) -> KernelVerdict {
        let active_leases = self.store.get_active_leases();
        let active_intents: Vec<SPOTriple> = active_leases.iter().map(SPOTriple::from).collect();
        let snapshot = StateSnapshot {
            active_leases,
            active_intents,
            priorities: self.store.get_priorities(),
        };

        KlockKernel::execute(&snapshot, manifest)
    }

    /// Acquire a lease on a resource.
    pub fn acquire_lease(
        &mut self,
        agent_id: &str,
        session_id: &str,
        resource_type: &str,
        resource_path: &str,
        predicate: &str,
        ttl: u64,
    ) -> LeaseResult {
        let resource = ResourceRef::new(parse_resource_type(resource_type), resource_path);
        let pred = parse_predicate(predicate);
        let now = now_ms();

        self.store
            .acquire(agent_id, session_id, resource, pred, ttl, now)
    }

    /// Release a held lease by its ID.
    pub fn release_lease(&mut self, lease_id: &str) -> bool {
        self.store.release(lease_id)
    }

    /// Get all currently active leases.
    pub fn get_active_leases(&self) -> Vec<Lease> {
        self.store.get_active_leases()
    }

    /// Evict expired leases. Returns the number of leases evicted.
    pub fn evict_expired(&mut self) -> usize {
        let now = now_ms();
        self.store.evict_expired(now)
    }

    /// Heartbeat a lease to renew its TTL. Returns true if successful.
    pub fn heartbeat_lease(&mut self, lease_id: &str, now: u64) -> bool {
        self.store.heartbeat(lease_id, now)
    }

    /// True when the underlying storage has poisoned itself due to a
    /// non-recoverable error. When poisoned, acquires fail with
    /// `LeaseFailureReason::StorageUnavailable` and `/health` returns 503.
    pub fn storage_poisoned(&self) -> bool {
        self.store.is_poisoned()
    }

    /// Generate a unique ID for intents/triples.
    pub fn next_id(&mut self) -> String {
        self.id_counter += 1;
        format!("klock_{}", self.id_counter)
    }
}

impl Default for KlockClient {
    fn default() -> Self {
        Self::new()
    }
}

// ─── Parsing Helpers ────────────────────────────────────────────────────────

pub fn parse_predicate(s: &str) -> Predicate {
    match s.to_uppercase().as_str() {
        "PROVIDES" => Predicate::Provides,
        "CONSUMES" => Predicate::Consumes,
        "MUTATES" => Predicate::Mutates,
        "DELETES" => Predicate::Deletes,
        "DEPENDS_ON" => Predicate::DependsOn,
        "RENAMES" => Predicate::Renames,
        _ => Predicate::Consumes, // Safe default
    }
}

pub fn parse_resource_type(s: &str) -> ResourceType {
    match s.to_uppercase().as_str() {
        "FILE" => ResourceType::File,
        "SYMBOL" => ResourceType::Symbol,
        "API_ENDPOINT" => ResourceType::ApiEndpoint,
        "DATABASE_TABLE" => ResourceType::DatabaseTable,
        "CONFIG_KEY" => ResourceType::ConfigKey,
        _ => ResourceType::File, // Safe default
    }
}
