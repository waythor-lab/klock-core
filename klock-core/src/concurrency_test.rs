//! Concurrency stress tests. These guard the property that under concurrent
//! contention on the same resource, exactly one acquire succeeds and the
//! rest get a typed Wait/Die failure — never a silent double-grant and
//! never a panic.
//!
//! These tests use the in-memory store wrapped in a Mutex<KlockClient> to
//! mirror the deployment shape used by `klock-cli`.

#[cfg(test)]
mod tests {
    use crate::client::KlockClient;
    use crate::types::{LeaseFailureReason, LeaseResult};
    use std::sync::{Arc, Mutex};
    use std::thread;

    #[test]
    fn one_hundred_concurrent_acquires_grant_exactly_one() {
        let mut seed = KlockClient::new();
        // Register 100 distinct agents with distinct priorities so the
        // scheduler has a strict order and the wait-die decision is fully
        // deterministic.
        for i in 0..100u64 {
            seed.register_agent(&format!("agent_{:03}", i), i * 10);
        }

        let client = Arc::new(Mutex::new(seed));

        let mut handles = Vec::with_capacity(100);
        for i in 0..100u64 {
            let client = Arc::clone(&client);
            handles.push(thread::spawn(move || {
                let mut c = client.lock().unwrap();
                c.acquire_lease(
                    &format!("agent_{:03}", i),
                    "session",
                    "FILE",
                    "/src/contended.ts",
                    "MUTATES",
                    60_000,
                )
            }));
        }

        let results: Vec<LeaseResult> = handles
            .into_iter()
            .map(|h| h.join().expect("thread panicked"))
            .collect();

        let successes = results
            .iter()
            .filter(|r| matches!(r, LeaseResult::Success { .. }))
            .count();
        let coordination_failures = results
            .iter()
            .filter(|r| {
                matches!(
                    r,
                    LeaseResult::Failure {
                        reason: LeaseFailureReason::Wait | LeaseFailureReason::Die,
                        ..
                    }
                )
            })
            .count();

        assert_eq!(
            successes, 1,
            "exactly one acquire must succeed under contention"
        );
        assert_eq!(
            coordination_failures,
            results.len() - 1,
            "all other acquires must report Wait or Die — never silent grant"
        );
    }

    /// Tokio variant: same property, exercised through the async lock that
    /// `klock-cli` actually uses in production.
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn one_hundred_concurrent_async_acquires_grant_exactly_one() {
        let mut seed = KlockClient::new();
        for i in 0..100u64 {
            seed.register_agent(&format!("agent_{:03}", i), i * 10);
        }

        let client = Arc::new(tokio::sync::Mutex::new(seed));

        let mut handles = Vec::with_capacity(100);
        for i in 0..100u64 {
            let client = Arc::clone(&client);
            handles.push(tokio::spawn(async move {
                let mut c = client.lock().await;
                c.acquire_lease(
                    &format!("agent_{:03}", i),
                    "session",
                    "FILE",
                    "/src/contended.ts",
                    "MUTATES",
                    60_000,
                )
            }));
        }

        let mut results = Vec::with_capacity(100);
        for h in handles {
            results.push(h.await.expect("task panicked"));
        }

        let successes = results
            .iter()
            .filter(|r| matches!(r, LeaseResult::Success { .. }))
            .count();
        assert_eq!(successes, 1, "tokio path must also grant exactly one");
    }
}
