#[cfg(test)]
mod tests {
    use crate::state::{IntentManifest, KernelVerdictStatus, KlockKernel, StateSnapshot};
    use crate::types::{Confidence, Lease, Predicate, ResourceRef, ResourceType, SPOTriple};
    use std::collections::HashMap;

    fn create_triple(agent_id: &str, predicate: Predicate, res_path: &str) -> SPOTriple {
        SPOTriple {
            id: format!("t_{}", agent_id),
            subject: agent_id.to_string(),
            predicate,
            object: ResourceRef::new(ResourceType::File, res_path),
            timestamp: 1000,
            confidence: Confidence::High,
            session_id: "s1".to_string(),
        }
    }

    fn create_lease(agent_id: &str, predicate: Predicate, res_path: &str) -> Lease {
        Lease::new(
            format!("l_{}", agent_id),
            agent_id.to_string(),
            "s_x".to_string(), // different session to force conflict
            ResourceRef::new(ResourceType::File, res_path),
            predicate,
            5000,
            1000,
        )
    }

    #[test]
    fn test_kernel_execute_granted() {
        let state = StateSnapshot {
            active_leases: vec![],
            active_intents: vec![],
            priorities: HashMap::new(),
        };

        let manifest = IntentManifest {
            session_id: "s1".to_string(),
            agent_id: "agent_a".to_string(),
            intents: vec![create_triple("agent_a", Predicate::Mutates, "/src/app.ts")],
        };

        let verdict = KlockKernel::execute(&state, &manifest);
        assert_eq!(verdict.status, KernelVerdictStatus::Granted);
        assert!(verdict.conflicts.is_empty());
    }

    #[test]
    fn test_kernel_execute_die() {
        let mut priorities = HashMap::new();
        priorities.insert("agent_older".to_string(), 100);
        priorities.insert("agent_younger".to_string(), 200);

        let state = StateSnapshot {
            active_leases: vec![create_lease("agent_older", Predicate::Mutates, "/src/app.ts")],
            active_intents: vec![],
            priorities,
        };

        let manifest = IntentManifest {
            session_id: "s2".to_string(),
            agent_id: "agent_younger".to_string(),
            intents: vec![create_triple(
                "agent_younger",
                Predicate::Mutates,
                "/src/app.ts",
            )],
        };

        let verdict = KlockKernel::execute(&state, &manifest);
        assert_eq!(verdict.status, KernelVerdictStatus::Die);
        assert!(!verdict.conflicts.is_empty());
        assert!(verdict.retry_after_ms.is_some());
    }

    #[test]
    fn test_kernel_execute_wait() {
        let mut priorities = HashMap::new();
        priorities.insert("agent_older".to_string(), 100);
        priorities.insert("agent_younger".to_string(), 200);

        let state = StateSnapshot {
            active_leases: vec![create_lease(
                "agent_younger",
                Predicate::Mutates,
                "/src/app.ts",
            )],
            active_intents: vec![],
            priorities,
        };

        let manifest = IntentManifest {
            session_id: "s2".to_string(),
            agent_id: "agent_older".to_string(),
            intents: vec![create_triple(
                "agent_older",
                Predicate::Mutates,
                "/src/app.ts",
            )],
        };

        let verdict = KlockKernel::execute(&state, &manifest);
        assert_eq!(verdict.status, KernelVerdictStatus::Wait);
        assert_eq!(verdict.held_by, Some("agent_younger".to_string()));
    }
}
