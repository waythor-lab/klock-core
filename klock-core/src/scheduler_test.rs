#[cfg(test)]
mod tests {
    use crate::scheduler::{VerdictStatus, WaitDieScheduler};
    use crate::types::{Lease, Predicate, ResourceRef, ResourceType};
    use std::collections::HashMap;

    fn create_lease(agent_id: &str, predicate: Predicate) -> Lease {
        Lease::new(
            "l1".to_string(),
            agent_id.to_string(),
            "s1".to_string(),
            ResourceRef::new(ResourceType::File, "/src/test.ts"),
            predicate,
            5000,
            1000,
        )
    }

    #[test]
    fn test_wait_die_older_waits() {
        let mut priorities = HashMap::new();
        priorities.insert("older".to_string(), 100);
        priorities.insert("younger".to_string(), 200);

        let active = vec![create_lease("younger", Predicate::Mutates)];

        let verdict = WaitDieScheduler::decide(
            "older",
            Predicate::Mutates, // Conflicts with Mutates
            &ResourceRef::new(ResourceType::File, "/src/test.ts"),
            &active,
            &priorities,
        );

        assert_eq!(verdict.status, VerdictStatus::Wait);
    }

    #[test]
    fn test_wait_die_younger_dies() {
        let mut priorities = HashMap::new();
        priorities.insert("older".to_string(), 100);
        priorities.insert("younger".to_string(), 200);

        let active = vec![create_lease("older", Predicate::Mutates)];

        let verdict = WaitDieScheduler::decide(
            "younger",
            Predicate::Mutates, // Conflicts with Mutates
            &ResourceRef::new(ResourceType::File, "/src/test.ts"),
            &active,
            &priorities,
        );

        assert_eq!(verdict.status, VerdictStatus::Die);
    }

    /// Equal priority + lexicographically smaller agent_id wins (waits).
    /// This prevents the symmetric DIE-loop that v0.1.2 produced when
    /// two agents shared a registration timestamp.
    #[test]
    fn test_equal_priority_lower_agent_id_waits() {
        let mut priorities = HashMap::new();
        priorities.insert("alpha".to_string(), 100);
        priorities.insert("beta".to_string(), 100);

        let active = vec![create_lease("beta", Predicate::Mutates)];

        let verdict = WaitDieScheduler::decide(
            "alpha", // (100, "alpha") < (100, "beta")
            Predicate::Mutates,
            &ResourceRef::new(ResourceType::File, "/src/test.ts"),
            &active,
            &priorities,
        );

        assert_eq!(verdict.status, VerdictStatus::Wait);
    }

    /// Equal priority + lexicographically larger agent_id loses (dies).
    /// Mirror of the previous test to confirm the ordering is antisymmetric.
    #[test]
    fn test_equal_priority_higher_agent_id_dies() {
        let mut priorities = HashMap::new();
        priorities.insert("alpha".to_string(), 100);
        priorities.insert("beta".to_string(), 100);

        let active = vec![create_lease("alpha", Predicate::Mutates)];

        let verdict = WaitDieScheduler::decide(
            "beta", // (100, "beta") > (100, "alpha")
            Predicate::Mutates,
            &ResourceRef::new(ResourceType::File, "/src/test.ts"),
            &active,
            &priorities,
        );

        assert_eq!(verdict.status, VerdictStatus::Die);
    }

    /// Strict priority inequality should still decide regardless of agent_id ordering
    /// (regression: ensure we didn't make agent_id dominate priority).
    #[test]
    fn test_priority_dominates_agent_id() {
        let mut priorities = HashMap::new();
        // "zzz" is lex-greater than "aaa" but priority 100 < 200, so zzz wins.
        priorities.insert("zzz".to_string(), 100);
        priorities.insert("aaa".to_string(), 200);

        let active = vec![create_lease("aaa", Predicate::Mutates)];

        let verdict = WaitDieScheduler::decide(
            "zzz",
            Predicate::Mutates,
            &ResourceRef::new(ResourceType::File, "/src/test.ts"),
            &active,
            &priorities,
        );

        assert_eq!(verdict.status, VerdictStatus::Wait);
    }
}
