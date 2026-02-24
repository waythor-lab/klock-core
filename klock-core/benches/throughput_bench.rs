use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};

use klock_core::client::KlockClient;
use klock_core::infrastructure::LeaseStore;
use klock_core::infrastructure_in_memory::InMemoryLeaseStore;
use klock_core::types::*;

fn bench_lease_acquire_release(c: &mut Criterion) {
    c.bench_function("lease_acquire_release_cycle", |b| {
        b.iter(|| {
            let mut client = KlockClient::new();
            client.register_agent("agent-1", 100);

            let result = client.acquire_lease(
                "agent-1", "s1", "FILE", "/app.ts", "MUTATES", 5000,
            );

            if let LeaseResult::Success { lease } = &result {
                client.release_lease(&lease.id);
            }
        })
    });
}

fn bench_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("lease_throughput");

    for agent_count in [10, 50, 100] {
        group.bench_with_input(
            BenchmarkId::new("agents", agent_count),
            &agent_count,
            |b, &count| {
                b.iter(|| {
                    let mut store = InMemoryLeaseStore::new();

                    // Register N agents with unique priorities
                    for i in 0..count {
                        store.register_agent_priority(format!("agent-{}", i), i as u64);
                    }

                    // Each agent acquires a lease on a different file
                    for i in 0..count {
                        let resource = ResourceRef::new(ResourceType::File, &format!("/file_{}.ts", i));
                        store.acquire(
                            &format!("agent-{}", i),
                            "s1",
                            resource,
                            Predicate::Mutates,
                            5000,
                            1000,
                        );
                    }

                    black_box(store.get_active_leases().len())
                })
            },
        );
    }

    group.finish();
}

fn bench_eviction(c: &mut Criterion) {
    c.bench_function("evict_1000_expired", |b| {
        b.iter(|| {
            let mut store = InMemoryLeaseStore::new();

            for i in 0..1000 {
                store.register_agent_priority(format!("a{}", i), i as u64);
                let resource = ResourceRef::new(ResourceType::File, &format!("/f{}.ts", i));
                store.acquire(&format!("a{}", i), "s1", resource, Predicate::Consumes, 100, 1000);
            }

            // Evict all (now > expires_at)
            black_box(store.evict_expired(99999))
        })
    });
}

criterion_group!(benches, bench_lease_acquire_release, bench_throughput, bench_eviction);
criterion_main!(benches);
