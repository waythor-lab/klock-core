#[cfg(test)]
mod tests {
    use crate::infrastructure::LeaseStore;
    use crate::infrastructure_in_memory::InMemoryLeaseStore;
    use crate::types::{LeaseFailureReason, LeaseResult, Predicate, ResourceRef, ResourceType};

    #[test]
    fn test_in_memory_store_acquire_and_release() {
        let mut store = InMemoryLeaseStore::new();
        store.register_agent_priority("agent_1".to_string(), 100);

        let res = ResourceRef::new(ResourceType::File, "/test");

        // Acquire
        let result = store.acquire("agent_1", "session_1", res.clone(), Predicate::Mutates, 5000, 1000);
        let lease = match result {
            LeaseResult::Success { lease } => lease,
            _ => panic!("Expected Success"),
        };

        assert_eq!(store.get_active_leases().len(), 1);

        // Release
        assert!(store.release(&lease.id));
        assert_eq!(store.get_active_leases().len(), 0);
    }

    #[test]
    fn test_in_memory_store_wait_die_enforcement() {
        let mut store = InMemoryLeaseStore::new();
        store.register_agent_priority("older".to_string(), 100);
        store.register_agent_priority("younger".to_string(), 200);

        let res = ResourceRef::new(ResourceType::File, "/test");

        // Older acquires a Mutates lease
        assert!(matches!(
            store.acquire("older", "s1", res.clone(), Predicate::Mutates, 5000, 1000),
            LeaseResult::Success { .. }
        ));

        // Younger tries to acquire a Mutates lease -> Should DIE
        let result = store.acquire("younger", "s2", res.clone(), Predicate::Mutates, 5000, 1000);
        assert!(matches!(
            result,
            LeaseResult::Failure { reason: LeaseFailureReason::Die, .. }
        ));
    }

    #[test]
    fn test_in_memory_store_eviction() {
         let mut store = InMemoryLeaseStore::new();
         store.register_agent_priority("agent_1".to_string(), 100);
         let res = ResourceRef::new(ResourceType::File, "/test");

         // Acquire at t=1000, ttl=5000 -> expires at 6000
         let _ = store.acquire("agent_1", "session_1", res, Predicate::Provides, 5000, 1000);
         
         assert_eq!(store.get_active_leases().len(), 1);

         // Evict at t=5000 (not expired yet)
         assert_eq!(store.evict_expired(5000), 0);
         assert_eq!(store.get_active_leases().len(), 1);

         // Evict at t=7000 (expired!)
         assert_eq!(store.evict_expired(7000), 1);
         assert_eq!(store.get_active_leases().len(), 0);
    }
}
