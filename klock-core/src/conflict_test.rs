#[cfg(test)]
mod tests {
    use crate::conflict::{ConflictEngine, ConflictResult};
    use crate::types::{Confidence, Predicate, ResourceRef, ResourceType, SPOTriple};

    // =========================================================================
    // Helper
    // =========================================================================
    fn make_triple(agent: &str, pred: Predicate, path: &str, session: &str) -> SPOTriple {
        SPOTriple {
            id: format!("t_{}_{}", agent, path),
            subject: agent.to_string(),
            predicate: pred,
            object: ResourceRef::new(ResourceType::File, path),
            timestamp: 1000,
            confidence: Confidence::High,
            session_id: session.to_string(),
        }
    }

    // =========================================================================
    // O(1) Matrix — Pair-level tests
    // =========================================================================

    #[test]
    fn consumes_consumes_compatible() {
        // Two reads should NOT conflict
        assert!(!ConflictEngine::check_pair(Predicate::Consumes, Predicate::Consumes));
    }

    #[test]
    fn consumes_mutates_conflicts() {
        assert!(ConflictEngine::check_pair(Predicate::Consumes, Predicate::Mutates));
        assert!(ConflictEngine::check_pair(Predicate::Mutates, Predicate::Consumes));
    }

    #[test]
    fn mutates_mutates_conflicts() {
        assert!(ConflictEngine::check_pair(Predicate::Mutates, Predicate::Mutates));
    }

    #[test]
    fn deletes_everything_conflicts() {
        // DELETE conflicts with every other predicate
        for pred in [
            Predicate::Provides,
            Predicate::Consumes,
            Predicate::Mutates,
            Predicate::Deletes,
            Predicate::DependsOn,
            Predicate::Renames,
        ] {
            assert!(
                ConflictEngine::check_pair(Predicate::Deletes, pred),
                "Deletes should conflict with {:?}",
                pred
            );
            assert!(
                ConflictEngine::check_pair(pred, Predicate::Deletes),
                "{:?} should conflict with Deletes",
                pred
            );
        }
    }

    #[test]
    fn provides_consumes_compatible() {
        // Creating a resource while another reads it is safe
        assert!(!ConflictEngine::check_pair(Predicate::Provides, Predicate::Consumes));
        assert!(!ConflictEngine::check_pair(Predicate::Consumes, Predicate::Provides));
    }

    #[test]
    fn depends_on_consumes_compatible() {
        // Dependency with read is safe
        assert!(!ConflictEngine::check_pair(Predicate::DependsOn, Predicate::Consumes));
        assert!(!ConflictEngine::check_pair(Predicate::Consumes, Predicate::DependsOn));
    }

    #[test]
    fn depends_on_mutates_conflicts() {
        // If you depend on something someone is mutating, that's a conflict
        assert!(ConflictEngine::check_pair(Predicate::DependsOn, Predicate::Mutates));
        assert!(ConflictEngine::check_pair(Predicate::Mutates, Predicate::DependsOn));
    }

    #[test]
    fn renames_everything_conflicts() {
        // RENAME conflicts with everything
        for pred in [
            Predicate::Provides,
            Predicate::Consumes,
            Predicate::Mutates,
            Predicate::Deletes,
            Predicate::DependsOn,
            Predicate::Renames,
        ] {
            assert!(
                ConflictEngine::check_pair(Predicate::Renames, pred),
                "Renames should conflict with {:?}",
                pred
            );
        }
    }

    // =========================================================================
    // Full triple check tests
    // =========================================================================

    #[test]
    fn check_no_existing_triples() {
        let new = make_triple("agent_a", Predicate::Mutates, "/src/app.ts", "s1");
        assert_eq!(ConflictEngine::check(&new, &[]), ConflictResult::Ok);
    }

    #[test]
    fn check_same_agent_same_session_no_conflict() {
        let existing = make_triple("agent_a", Predicate::Mutates, "/src/app.ts", "s1");
        let new = make_triple("agent_a", Predicate::Mutates, "/src/app.ts", "s1");
        assert_eq!(ConflictEngine::check(&new, &[existing]), ConflictResult::Ok);
    }

    #[test]
    fn check_different_resource_no_conflict() {
        let existing = make_triple("agent_a", Predicate::Mutates, "/src/foo.ts", "s1");
        let new = make_triple("agent_b", Predicate::Mutates, "/src/bar.ts", "s2");
        assert_eq!(ConflictEngine::check(&new, &[existing]), ConflictResult::Ok);
    }

    #[test]
    fn check_different_agent_same_resource_detects_conflict() {
        let existing = make_triple("agent_a", Predicate::Mutates, "/src/app.ts", "s1");
        let new = make_triple("agent_b", Predicate::Mutates, "/src/app.ts", "s2");
        assert!(matches!(
            ConflictEngine::check(&new, &[existing]),
            ConflictResult::Conflict { .. }
        ));
    }
}
