# Klock Tiny Repro

This is the smallest Klock demo in the repo.

It shows one thing only:

- **without coordination**: two workers silently overwrite each other
- **with Klock**: the same two workers serialize safely

No OpenRouter. No LangChain. No external APIs.

## What This Demonstrates

Both workers do the same classic pattern:

1. read shared state
2. modify it locally
3. write it back

Without coordination, both workers read the same initial state and one write silently overwrites the other.

With Klock, one worker acquires the lease first and the other waits or retries until it can proceed safely.

## Files

- `race_condition.py` — deterministic silent overwrite without coordination
- `klock_fixed.py` — same workflow protected by Klock via the local HTTP server

## 1. Run The Failure Case

```bash
python race_condition.py
```

Expected result:

- both workers report success
- final state contains **1** entry instead of **2**

## 2. Run The Klock-Protected Case

Start the local Klock server from `Klock-OpenSource/`:

```bash
cargo run --release -p klock-cli -- serve
```

Then in this directory run:

```bash
python klock_fixed.py
```

Expected result:

- one worker acquires the lease first
- the other waits or retries
- final state contains **2** entries

## Why This Asset Exists

This example is meant for:

- forum posts
- GitHub issue replies
- quick demos

It is intentionally smaller than the LangChain / OpenRouter demos so the failure mode is obvious in a few seconds.
