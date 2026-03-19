# Benchmarks and Proof

Klock OSS v1 needs two kinds of proof:

- **product proof**: it turns silent overwrite in a shared repo into explicit coordination outcomes
- **engine proof**: the coordination kernel is fast enough to sit in front of agent tool calls

## Product proof

Run the deterministic repo coordination demos:

```bash
cd Klock-OpenSource/examples/oss_v1
python3 without_klock.py
python3 with_klock.py
python3 wait_die_trace.py
```

What these prove:

- `without_klock.py`: silent overwrite is real and easy to reproduce
- `with_klock.py`: the same file survives concurrent edits when both agents cooperate with Klock
- `wait_die_trace.py`: `GRANT`, `WAIT`, and `DIE` are visible and understandable

## Engine benchmarks

Run the Rust kernel benchmarks:

```bash
cd Klock-OpenSource
cargo bench -p klock-core
```

Criterion reports are written to `target/criterion/report/index.html`.

## Current performance summary

| Operation | Latency | Notes |
|-----------|---------|-------|
| Conflict check pair | ~1 ns | Single compatibility lookup |
| Conflict check with 1000 triples | ~339 ns | O(1) conflict matrix behavior |
| Wait-Die scheduling decision | ~25 ns | Priority comparison |
| Full kernel execute | ~500 ns | Intent to verdict pipeline |
| Lease acquire + release | ~670 ns | End-to-end local kernel flow |

## Why both proofs matter

Benchmarks alone do not prove product value. The public OSS v1 claim is not “Klock is fast.” The claim is:

> multiple coding agents can coordinate shared repo edits without silently overwriting each other

That is why the repo ships both the kernel benchmarks and the local repo/workspace proof scripts.
