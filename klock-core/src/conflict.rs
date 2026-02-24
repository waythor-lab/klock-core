use crate::types::{Lease, Predicate, SPOTriple};

/// Represents the outcome of a conflict check
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConflictResult {
    /// No conflict found
    Ok,
    /// A conflict was detected
    Conflict { reason: String },
}

/// A pure engine for O(1) conflict detection using precomputed compatibility matrices.
pub struct ConflictEngine;

impl ConflictEngine {
    /// Central 6x6 Compatibility Matrix based on Wait-Die semantics.
    /// Rows: Existing Predicate (Held)
    /// Cols: New Predicate (Requesting)
    /// True = Compatible (No Conflict)
    /// False = Incompatible (Conflict)
    /// 
    /// Order: Provides(0), Consumes(1), Mutates(2), Deletes(3), DependsOn(4), Renames(5)
    #[rustfmt::skip]
    const MATRIX: [[bool; 6]; 6] = [
        //          Prov   Cons   Mut    Del    Dep    Ren
        /* Prov */ [false, true,  false, false, true,  false],
        /* Cons */ [true,  true,  false, false, true,  false],
        /* Mut  */ [false, false, false, false, false, false],
        /* Del  */ [false, false, false, false, false, false],
        /* Dep  */ [true,  true,  false, false, true,  false],
        /* Ren  */ [false, false, false, false, false, false],
    ];

    /// O(1) check if two predicates conflict
    pub fn check_pair(held: Predicate, requesting: Predicate) -> bool {
        // We look up the matrix. It returns true if COMPATIBLE.
        // Therefore, it CONFLICTS if the matrix returns FALSE.
        !Self::MATRIX[held.to_index()][requesting.to_index()]
    }

    /// Checks if a new intent conflicts with any existing intents.
    pub fn check(new_triple: &SPOTriple, existing_triples: &[SPOTriple]) -> ConflictResult {
        let key = new_triple.object.key();

        for existing in existing_triples {
            // Skip if they are for a different resource
            if existing.object.key() != key {
                continue;
            }

            // Skip if it is the same agent in the same session (reentrant lock logic)
            if existing.subject == new_triple.subject && existing.session_id == new_triple.session_id {
                continue;
            }

            if Self::check_pair(existing.predicate, new_triple.predicate) {
                return ConflictResult::Conflict {
                    reason: format!(
                        "Agent {}'s {:?} operation conflicts with Agent {}'s held {:?} operation on {:?}",
                        new_triple.subject,
                        new_triple.predicate,
                        existing.subject,
                        existing.predicate,
                        new_triple.object
                    ),
                };
            }
        }

        ConflictResult::Ok
    }

    /// Checks if a requested predicate conflicts with any active leases
    pub fn check_against_leases(
        requesting_agent: &str,
        requesting_session: &str,
        requesting_predicate: Predicate,
        resource_key: &str,
        active_leases: &[Lease],
    ) -> ConflictResult {
        for lease in active_leases {
            if lease.resource.key() != resource_key {
                continue;
            }

            if lease.agent_id == requesting_agent && lease.session_id == requesting_session {
                continue;
            }

            if Self::check_pair(lease.predicate, requesting_predicate) {
                return ConflictResult::Conflict {
                    reason: format!(
                        "Conflict: {:?} vs held {:?}",
                        requesting_predicate, lease.predicate
                    ),
                };
            }
        }

        ConflictResult::Ok
    }
}
