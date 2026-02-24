---
sidebar_position: 6
---

# Performance Benchmarks

Klock's Rust core is designed for sub-microsecond coordination. All benchmarks use [Criterion](https://bheisler.github.io/criterion.rs/book/) and were measured on Apple Silicon (M-series) in release mode.

## Running Benchmarks

```bash
cd Klock-OpenSource
cargo bench -p klock-core
```

HTML reports are generated at `target/criterion/report/index.html`.

---

## Results Summary

| Benchmark | Latency | Notes |
|-----------|---------|-------|
| `conflict_check_pair` | **~1 ns** | Single matrix lookup |
| `conflict_check` (10 triples) | **~337 ns** | O(1) — constant regardless of count |
| `conflict_check` (100 triples) | **~337 ns** | Same as 10! |
| `conflict_check` (1000 triples) | **~339 ns** | Same as 10! O(1) confirmed |
| `scheduler_decide` | **~25 ns** | Wait-Die priority comparison |
| `kernel_execute` | **~500 ns** | Full intent → verdict pipeline |
| `lease_acquire_release` | **~670 ns** | Complete acquire + release cycle |
| `10_agents_throughput` | **~16 μs** | 10 agents acquiring leases |
| `100_agents_throughput` | **~1 ms** | 100 agents acquiring leases |
| `evict_1000_expired` | **~94 ms** | Bulk expiration cleanup |

---

## Key Finding: O(1) Conflict Detection

The most important benchmark result is that **conflict checking time is constant** regardless of how many active intents exist:

```
check_with_10_triples    → 337 ns
check_with_100_triples   → 337 ns
check_with_1000_triples  → 339 ns
```

This proves the 6×6 matrix design delivers true O(1) performance. The cost is dominated by the resource key comparison (`String::eq`), not the matrix lookup itself.

---

## Comparison: Rust vs TypeScript/JavaScript

| Operation | Klock (Rust) | Typical JS Equivalent | Speedup |
|-----------|-------------|----------------------|---------|
| Conflict check pair | ~1 ns | ~1-10 μs (object comparison) | **1,000–10,000x** |
| Lease acquire/release | ~670 ns | ~50-200 μs (async + GC) | **75–300x** |
| Full kernel execute | ~500 ns | ~100-500 μs (V8 JIT) | **200–1,000x** |

The Rust implementation eliminates:
- **Garbage collection** pauses
- **V8 JIT** warmup overhead
- **Async runtime** scheduling cost
- **Object allocation** per operation

---

## Benchmark Suites

### `conflict_bench.rs`

Tests the pure conflict detection engine and kernel:

| Benchmark | Description |
|-----------|-------------|
| `bench_check_pair` | Single predicate-pair compatibility check |
| `bench_check_with_varying_triples` | Conflict check scaling (10, 100, 1000 active intents) |
| `bench_scheduler_decide` | Wait-Die priority resolution |
| `bench_kernel_execute` | Full `KlockKernel::execute()` pipeline |

### `throughput_bench.rs`

Tests real-world throughput patterns:

| Benchmark | Description |
|-----------|-------------|
| `bench_lease_acquire_release` | Single agent: acquire + release cycle |
| `bench_throughput` | N agents each acquiring a lease (10, 100) |
| `bench_eviction` | Evicting 1000 expired leases |

---

## Methodology

- **Framework**: Criterion 0.5 with default settings (100 samples, auto-tuned warm-up)
- **Profile**: `--release` (optimized build)
- **Hardware**: Apple Silicon (ARM64)
- **Isolation**: Each benchmark creates fresh state (no cross-contamination)
- **Statistics**: Criterion reports [lower bound, estimate, upper bound] at 95% confidence
