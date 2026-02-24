use crate::conflict::{ConflictEngine, ConflictResult};
use crate::scheduler::{WaitDieScheduler, VerdictStatus};
use crate::types::{Lease, SPOTriple};
use std::collections::HashMap;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntentManifest {
    pub session_id: String,
    pub agent_id: String,
    pub intents: Vec<SPOTriple>,
}

#[derive(Debug, Clone)]
pub struct StateSnapshot {
    pub active_leases: Vec<Lease>,
    pub active_intents: Vec<SPOTriple>,
    pub priorities: HashMap<String, u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum KernelVerdictStatus {
    Granted,
    Wait,
    Die,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KernelVerdict {
    pub agent_id: String,
    pub session_id: String,
    pub status: KernelVerdictStatus,
    pub reason: Option<String>,
    pub held_by: Option<String>,
    pub conflicts: Vec<String>,
    pub retry_after_ms: Option<u64>,
}

pub struct KlockKernel;

impl KlockKernel {
    pub fn execute(state: &StateSnapshot, manifest: &IntentManifest) -> KernelVerdict {
        let mut conflicts = Vec::new();
        let mut worst_status = KernelVerdictStatus::Granted;
        let mut return_reason = None;
        let mut return_held_by = None;
        let mut return_retry = None;

        for intent in &manifest.intents {
            // 1. Check for Conflicts via Conflict Engine
            let conflict_result = ConflictEngine::check(intent, &state.active_intents);

            if let ConflictResult::Conflict { reason } = conflict_result {
                conflicts.push(reason.clone());

                // 2. Resolve via Scheduler
                let scheduler_verdict = WaitDieScheduler::decide(
                    &manifest.agent_id,
                    intent.predicate,
                    &intent.object,
                    &state.active_leases,
                    &state.priorities,
                );

                match scheduler_verdict.status {
                    VerdictStatus::Wait => {
                        if worst_status != KernelVerdictStatus::Die {
                            worst_status = KernelVerdictStatus::Wait;
                            return_reason = scheduler_verdict.reason;
                            return_held_by = scheduler_verdict.held_by;
                        }
                    }
                    VerdictStatus::Die => {
                        worst_status = KernelVerdictStatus::Die;
                        return_reason = scheduler_verdict.reason;
                        return_held_by = scheduler_verdict.held_by;
                        return_retry = scheduler_verdict.retry_after_ms;
                    }
                    VerdictStatus::Granted => {}
                }
            } else {
                // No explicit intent conflicts, check against active leases directly
                let lease_verdict = WaitDieScheduler::decide(
                    &manifest.agent_id,
                    intent.predicate,
                    &intent.object,
                    &state.active_leases,
                    &state.priorities,
                );

                if lease_verdict.status != VerdictStatus::Granted {
                    conflicts.push(format!("Conflict with active lease on {:?}", intent.object));
                    match lease_verdict.status {
                        VerdictStatus::Wait => {
                            if worst_status != KernelVerdictStatus::Die {
                                worst_status = KernelVerdictStatus::Wait;
                                return_reason = lease_verdict.reason;
                                return_held_by = lease_verdict.held_by;
                            }
                        }
                        VerdictStatus::Die => {
                            worst_status = KernelVerdictStatus::Die;
                            return_reason = lease_verdict.reason;
                            return_held_by = lease_verdict.held_by;
                            return_retry = lease_verdict.retry_after_ms;
                        }
                        _ => {}
                    }
                }
            }
        }

        KernelVerdict {
            agent_id: manifest.agent_id.clone(),
            session_id: manifest.session_id.clone(),
            status: worst_status,
            reason: return_reason,
            held_by: return_held_by,
            conflicts,
            retry_after_ms: return_retry,
        }
    }
}
