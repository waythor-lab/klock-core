use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};

use klock_core::conflict::ConflictEngine;
use klock_core::scheduler::WaitDieScheduler;
use klock_core::state::{IntentManifest, KlockKernel, StateSnapshot};
use klock_core::types::*;

use std::collections::HashMap;

// ─── Helpers ────────────────────────────────────────────────────────────────

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

fn make_lease(agent: &str, pred: Predicate, path: &str) -> Lease {
    Lease::new(
        format!("l_{}_{}", agent, path),
        agent.to_string(),
        "s1".to_string(),
        ResourceRef::new(ResourceType::File, path),
        pred,
        5000,
        1000,
    )
}

// ─── Benchmarks ─────────────────────────────────────────────────────────────

fn bench_check_pair(c: &mut Criterion) {
    c.bench_function("conflict_check_pair", |b| {
        b.iter(|| {
            ConflictEngine::check_pair(
                black_box(Predicate::Mutates),
                black_box(Predicate::Mutates),
            )
        })
    });
}

fn bench_check_with_varying_triples(c: &mut Criterion) {
    let mut group = c.benchmark_group("conflict_check_triples");

    for count in [10, 100, 1000] {
        let existing: Vec<SPOTriple> = (0..count)
            .map(|i| make_triple(&format!("agent_{}", i), Predicate::Consumes, &format!("/file_{}.ts", i), "s1"))
            .collect();

        let new = make_triple("agent_new", Predicate::Mutates, "/file_0.ts", "s2");

        group.bench_with_input(BenchmarkId::from_parameter(count), &count, |b, _| {
            b.iter(|| ConflictEngine::check(black_box(&new), black_box(&existing)))
        });
    }

    group.finish();
}

fn bench_scheduler_decide(c: &mut Criterion) {
    let mut priorities = HashMap::new();
    priorities.insert("older".to_string(), 100_u64);
    priorities.insert("younger".to_string(), 200_u64);

    let active = vec![make_lease("older", Predicate::Mutates, "/app.ts")];
    let resource = ResourceRef::new(ResourceType::File, "/app.ts");

    c.bench_function("scheduler_decide", |b| {
        b.iter(|| {
            WaitDieScheduler::decide(
                black_box("younger"),
                black_box(Predicate::Mutates),
                black_box(&resource),
                black_box(&active),
                black_box(&priorities),
            )
        })
    });
}

fn bench_kernel_execute(c: &mut Criterion) {
    let mut priorities = HashMap::new();
    priorities.insert("older".to_string(), 100_u64);
    priorities.insert("younger".to_string(), 200_u64);

    let state = StateSnapshot {
        active_leases: vec![make_lease("older", Predicate::Mutates, "/app.ts")],
        active_intents: vec![make_triple("older", Predicate::Mutates, "/app.ts", "s1")],
        priorities,
    };

    let manifest = IntentManifest {
        session_id: "s2".to_string(),
        agent_id: "younger".to_string(),
        intents: vec![make_triple("younger", Predicate::Mutates, "/app.ts", "s2")],
    };

    c.bench_function("kernel_execute", |b| {
        b.iter(|| KlockKernel::execute(black_box(&state), black_box(&manifest)))
    });
}

criterion_group!(
    benches,
    bench_check_pair,
    bench_check_with_varying_triples,
    bench_scheduler_decide,
    bench_kernel_execute,
);
criterion_main!(benches);
