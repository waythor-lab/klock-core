use crate::conflict::ConflictEngine;
use crate::types::{Lease, Predicate, ResourceRef};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerdictStatus {
    Granted,
    Wait,
    Die,
}

#[derive(Debug, Clone)]
pub struct SchedulerVerdict {
    pub status: VerdictStatus,
    pub reason: Option<String>,
    pub held_by: Option<String>,
    pub retry_after_ms: Option<u64>,
}

pub struct WaitDieScheduler;

impl WaitDieScheduler {
    pub fn decide(
        requesting_agent_id: &str,
        requesting_predicate: Predicate,
        resource: &ResourceRef,
        active_leases: &[Lease],
        priorities: &HashMap<String, u64>,
    ) -> SchedulerVerdict {
        let key = resource.key();

        // 1. Find conflicting holders
        let mut conflicting_holders = Vec::new();
        for lease in active_leases {
            if lease.resource.key() == key
                && lease.agent_id != requesting_agent_id // Skip self
                && ConflictEngine::check_pair(lease.predicate, requesting_predicate)
            {
                conflicting_holders.push(lease);
            }
        }

        if conflicting_holders.is_empty() {
            return SchedulerVerdict {
                status: VerdictStatus::Granted,
                reason: None,
                held_by: None,
                retry_after_ms: None,
            };
        }

        // 2. Fetch requester priority (timestamp - lower is older/higher priority)
        let requester_priority = match priorities.get(requesting_agent_id) {
            Some(p) => *p,
            None => {
                return SchedulerVerdict {
                    status: VerdictStatus::Die,
                    reason: Some("Missing agent priority. Cannot ensure deadlock safety.".into()),
                    held_by: None,
                    retry_after_ms: Some(1000), // Base backoff
                };
            }
        };

        // 3. Apply Wait-Die logic against all conflicting holders
        for holder in conflicting_holders {
            let holder_priority = match priorities.get(&holder.agent_id) {
                Some(p) => *p,
                None => continue, // If holder has no priority, assume they are younger
            };

            if requester_priority < holder_priority {
                // Requester is OLDER (lower timestamp) -> WAIT
                return SchedulerVerdict {
                    status: VerdictStatus::Wait,
                    reason: Some(format!(
                        "Senior ({}) waiting for Junior ({}) to complete.",
                        requester_priority, holder_priority
                    )),
                    held_by: Some(holder.agent_id.clone()),
                    retry_after_ms: None,
                };
            } else {
                // Requester is YOUNGER (higher timestamp) -> DIE
                return SchedulerVerdict {
                    status: VerdictStatus::Die,
                    reason: Some(format!(
                        "Conflict: Senior ({}) vs Junior ({}). Junior must DIE.",
                        holder_priority, requester_priority
                    )),
                    held_by: Some(holder.agent_id.clone()),
                    retry_after_ms: Some(1000),
                };
            }
        }

        SchedulerVerdict {
            status: VerdictStatus::Granted,
            reason: None,
            held_by: None,
            retry_after_ms: None,
        }
    }
}
