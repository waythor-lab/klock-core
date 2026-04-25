//! Tests for the SQLite-backed lease store, focusing on the v0.1.3
//! fail-closed behavior:
//! - read failure -> store poisons + returns empty Vec (without panic)
//! - subsequent acquire short-circuits to StorageUnavailable
//! - register_agent_priority does not diverge in-memory vs disk on failure
//!
//! We simulate failure by dropping the underlying tables out from under the
//! store, which is the closest in-process analogue to a corrupted DB.

#[cfg(test)]
mod tests {
    use crate::infrastructure::LeaseStore;
    use crate::infrastructure_sqlite::SqliteLeaseStore;
    use crate::types::{LeaseFailureReason, LeaseResult, Predicate, ResourceRef, ResourceType};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    fn temp_db_path(label: &str) -> PathBuf {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let mut path = std::env::temp_dir();
        path.push(format!(
            "klock_sqlite_test_{}_{}_{}.db",
            label,
            std::process::id(),
            n
        ));
        // Ensure no leftover file from a previous run.
        let _ = std::fs::remove_file(&path);
        path
    }

    /// Cleanup helper to drop the DB file at end of test.
    struct CleanupOnDrop(PathBuf);
    impl Drop for CleanupOnDrop {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
            // WAL files
            let _ = std::fs::remove_file(self.0.with_extension("db-wal"));
            let _ = std::fs::remove_file(self.0.with_extension("db-shm"));
        }
    }

    #[test]
    fn happy_path_acquire_release_persists() {
        let path = temp_db_path("happy");
        let _cleanup = CleanupOnDrop(path.clone());

        let mut store = SqliteLeaseStore::open(path.to_str().unwrap()).expect("open");
        store.register_agent_priority("a".to_string(), 100);

        let res = store.acquire(
            "a",
            "s",
            ResourceRef::new(ResourceType::File, "/x"),
            Predicate::Mutates,
            60_000,
            1_000,
        );
        assert!(matches!(res, LeaseResult::Success { .. }));
        assert!(!store.is_poisoned());
    }

    /// Reading from a store whose `leases` table has been dropped must NOT
    /// panic. It must poison the store and return an empty Vec.
    #[test]
    fn dropped_leases_table_poisons_store_on_read() {
        let path = temp_db_path("read_fail");
        let _cleanup = CleanupOnDrop(path.clone());

        let mut store = SqliteLeaseStore::open(path.to_str().unwrap()).expect("open");
        store.register_agent_priority("a".to_string(), 100);

        // Sanity: read works before the drop.
        assert!(store.get_active_leases().is_empty());

        // Now break the schema. We open a separate connection so the
        // store's connection sees the broken schema on its next read.
        rusqlite::Connection::open(&path)
            .expect("second conn")
            .execute("DROP TABLE leases", [])
            .expect("drop table");

        let leases = store.get_active_leases();
        assert!(leases.is_empty(), "must not panic; must return empty");
        assert!(store.is_poisoned(), "must poison store after read failure");
    }

    /// After poisoning, acquire must short-circuit with StorageUnavailable.
    /// This is the fail-closed property.
    #[test]
    fn poisoned_store_fails_closed_on_acquire() {
        let path = temp_db_path("fail_closed");
        let _cleanup = CleanupOnDrop(path.clone());

        let mut store = SqliteLeaseStore::open(path.to_str().unwrap()).expect("open");
        store.register_agent_priority("a".to_string(), 100);

        rusqlite::Connection::open(&path)
            .expect("second conn")
            .execute("DROP TABLE leases", [])
            .expect("drop table");

        // Trigger poisoning via a read.
        let _ = store.get_active_leases();
        assert!(store.is_poisoned());

        let res = store.acquire(
            "a",
            "s",
            ResourceRef::new(ResourceType::File, "/x"),
            Predicate::Mutates,
            60_000,
            1_000,
        );
        match res {
            LeaseResult::Failure {
                reason: LeaseFailureReason::StorageUnavailable,
                ..
            } => {}
            other => panic!("expected StorageUnavailable, got {:?}", other),
        }
    }

    /// Insert failure path: drop the leases table after registration but
    /// before acquire. The acquire must surface StorageUnavailable, not
    /// pretend Success.
    #[test]
    fn insert_failure_returns_storage_unavailable_not_phantom_success() {
        let path = temp_db_path("insert_fail");
        let _cleanup = CleanupOnDrop(path.clone());

        let mut store = SqliteLeaseStore::open(path.to_str().unwrap()).expect("open");
        store.register_agent_priority("a".to_string(), 100);

        rusqlite::Connection::open(&path)
            .expect("second conn")
            .execute("DROP TABLE leases", [])
            .expect("drop table");

        // The first call goes through evict_expired -> get_active_leases
        // which will fail and poison the store; acquire should then return
        // StorageUnavailable rather than a Success the caller cannot trust.
        let res = store.acquire(
            "a",
            "s",
            ResourceRef::new(ResourceType::File, "/x"),
            Predicate::Mutates,
            60_000,
            1_000,
        );
        match res {
            LeaseResult::Failure {
                reason: LeaseFailureReason::StorageUnavailable,
                ..
            } => {}
            LeaseResult::Success { .. } => {
                panic!("acquire reported Success despite a broken DB — coordination breach")
            }
            other => panic!("expected StorageUnavailable, got {:?}", other),
        }
        assert!(store.is_poisoned());
    }

    /// register_agent_priority must only update the in-memory map when the
    /// disk write succeeds. Otherwise the in-memory priority "works" for
    /// one server lifetime and silently disappears on restart.
    #[test]
    fn register_agent_priority_does_not_diverge_on_failure() {
        let path = temp_db_path("priority_fail");
        let _cleanup = CleanupOnDrop(path.clone());

        let mut store = SqliteLeaseStore::open(path.to_str().unwrap()).expect("open");

        // Drop the priorities table to force the INSERT to fail.
        rusqlite::Connection::open(&path)
            .expect("second conn")
            .execute("DROP TABLE agent_priorities", [])
            .expect("drop table");

        store.register_agent_priority("a".to_string(), 100);

        // Verify divergence: the on-disk write failed, so the in-memory
        // map must not contain "a" either.
        let prios = store.get_priorities();
        assert!(
            !prios.contains_key("a"),
            "in-memory priority map must not diverge from disk on write failure (would silently regress on restart)"
        );
    }
}
