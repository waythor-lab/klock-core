//! Tests for KlockClient covering the v0.1.3 intent-lifecycle fix:
//! the kernel's `active_intents` is derived from active leases at snapshot
//! time, so declaring an intent does not accumulate state on the client.

#[cfg(test)]
mod tests {
    use crate::client::KlockClient;
    use crate::state::{IntentManifest, KernelVerdictStatus};
    use crate::types::{Confidence, Predicate, ResourceRef, ResourceType, SPOTriple};

    fn triple(id: &str, agent: &str, session: &str, predicate: Predicate, path: &str) -> SPOTriple {
        SPOTriple {
            id: id.to_string(),
            subject: agent.to_string(),
            predicate,
            object: ResourceRef::new(ResourceType::File, path),
            timestamp: 0,
            confidence: Confidence::High,
            session_id: session.to_string(),
        }
    }

    /// Declaring the same intent twice must not poison the kernel snapshot.
    /// Pre-fix: `active_intents` grew unboundedly each call.
    #[test]
    fn declare_intent_does_not_accumulate() {
        let mut client = KlockClient::new();
        client.register_agent("a", 100);

        let manifest = IntentManifest {
            session_id: "s1".to_string(),
            agent_id: "a".to_string(),
            intents: vec![triple(
                "i1",
                "a",
                "s1",
                Predicate::Mutates,
                "/src/auth.ts",
            )],
        };

        let v1 = client.declare_intent(&manifest);
        let v2 = client.declare_intent(&manifest);

        // Both verdicts should be Granted: there are no held leases, and the
        // kernel does not see "phantom" intents from the previous call.
        assert_eq!(v1.status, KernelVerdictStatus::Granted);
        assert_eq!(
            v2.status,
            KernelVerdictStatus::Granted,
            "second declare_intent must not see the first declaration as a held intent"
        );
    }

    /// After `acquire_lease`, the kernel snapshot must reflect the held lease
    /// as an active intent. After `release_lease`, the intent view must clear.
    #[test]
    fn release_lease_clears_derived_intent_view() {
        let mut client = KlockClient::new();
        client.register_agent("a", 100);
        client.register_agent("b", 200);

        // a holds a Mutates lease.
        let res = client.acquire_lease("a", "sa", "FILE", "/src/x.ts", "MUTATES", 60_000);
        let lease_id = match res {
            crate::types::LeaseResult::Success { lease } => lease.id,
            other => panic!("expected Success, got {:?}", other),
        };

        // b's declare_intent on the same resource should be denied (Wait or Die)
        // because the kernel sees a's lease as an active intent.
        let manifest_b = IntentManifest {
            session_id: "sb".to_string(),
            agent_id: "b".to_string(),
            intents: vec![triple(
                "ib",
                "b",
                "sb",
                Predicate::Mutates,
                "/src/x.ts",
            )],
        };
        let v_blocked = client.declare_intent(&manifest_b);
        assert_ne!(
            v_blocked.status,
            KernelVerdictStatus::Granted,
            "b should not be granted while a holds a conflicting lease"
        );

        // Now release a's lease; b should now succeed.
        assert!(client.release_lease(&lease_id));
        let v_after = client.declare_intent(&manifest_b);
        assert_eq!(
            v_after.status,
            KernelVerdictStatus::Granted,
            "after release the derived intent view must clear"
        );
    }

    /// Releasing an unknown lease ID must be a safe no-op.
    #[test]
    fn release_lease_unknown_id_returns_false() {
        let mut client = KlockClient::new();
        assert!(!client.release_lease("does-not-exist"));
    }

    /// Storage poison flag is false for the in-memory store.
    #[test]
    fn in_memory_store_is_never_poisoned() {
        let client = KlockClient::new();
        assert!(!client.storage_poisoned());
    }
}
