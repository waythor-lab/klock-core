use crate::scheduler::{VerdictStatus, WaitDieScheduler};
use crate::types::{Lease, LeaseFailureReason, LeaseResult, Predicate, ResourceRef};
use crate::infrastructure::LeaseStore;
use std::collections::HashMap;

pub struct InMemoryLeaseStore {
    // Map of Lease ID -> Lease
    leases: HashMap<String, Lease>,
    // Map of Agent ID -> Priority (Timestamp)
    priorities: HashMap<String, u64>,
}

impl InMemoryLeaseStore {
    pub fn new() -> Self {
        Self {
            leases: HashMap::new(),
            priorities: HashMap::new(),
        }
    }

    pub fn register_agent_priority(&mut self, agent_id: String, priority_timestamp: u64) {
        self.priorities.insert(agent_id, priority_timestamp);
    }

    pub fn get_priorities(&self) -> HashMap<String, u64> {
        self.priorities.clone()
    }
}

impl LeaseStore for InMemoryLeaseStore {
    fn acquire(
        &mut self,
        agent_id: &str,
        session_id: &str,
        resource: ResourceRef,
        predicate: Predicate,
        ttl: u64,
        now: u64,
    ) -> LeaseResult {
        // Clean up expired leases first
        self.evict_expired(now);

        let active_leases = self.get_active_leases();
        
        // 1. Check Wait-Die Scheduler
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
                existing_lease: None, // Simplified for now
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
                    resource,
                    predicate,
                    ttl,
                    now,
                );

                self.leases.insert(lease_id, lease.clone());

                LeaseResult::Success { lease }
            }
        }
    }

    fn release(&mut self, lease_id: &str) -> bool {
        if let Some(lease) = self.leases.get_mut(lease_id) {
            lease.state = crate::types::LeaseState::Released;
            true
        } else {
            false
        }
    }

    fn heartbeat(&mut self, lease_id: &str, now: u64) -> bool {
        if let Some(lease) = self.leases.get_mut(lease_id) {
            if lease.state == crate::types::LeaseState::Active {
                lease.last_heartbeat = now;
                lease.expires_at = now + lease.ttl;
                return true;
            }
        }
        false
    }

    fn get_active_leases(&self) -> Vec<Lease> {
        self.leases
            .values()
            .filter(|l| l.state == crate::types::LeaseState::Active)
            .cloned()
            .collect()
    }

    fn evict_expired(&mut self, now: u64) -> usize {
        let mut expired_count = 0;
        for lease in self.leases.values_mut() {
            if lease.state == crate::types::LeaseState::Active && lease.expires_at < now {
                lease.state = crate::types::LeaseState::Expired;
                expired_count += 1;
            }
        }
        expired_count
    }
}
