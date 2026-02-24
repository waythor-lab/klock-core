use crate::types::{Lease, LeaseResult, Predicate, ResourceRef};

// In a real system, these would likely return Results with specific error types
// and use async/await. For the core kernel representation, we keep it synchronous
// or abstracted behind a trait.

/// Defines the contract for lease storage backends.
pub trait LeaseStore {
    /// Attempt to acquire a lease on a resource
    fn acquire(
        &mut self,
        agent_id: &str,
        session_id: &str,
        resource: ResourceRef,
        predicate: Predicate,
        ttl: u64,
        now: u64,
    ) -> LeaseResult;

    /// Release an explicitly held lease
    fn release(&mut self, lease_id: &str) -> bool;

    /// Heartbeat an active lease to extend its TTL
    fn heartbeat(&mut self, lease_id: &str, now: u64) -> bool;

    /// Get all currently active leases
    fn get_active_leases(&self) -> Vec<Lease>;
    
    /// Evict expired leases based on the current time
    fn evict_expired(&mut self, now: u64) -> usize;
}

