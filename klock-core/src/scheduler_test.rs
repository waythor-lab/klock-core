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
}
