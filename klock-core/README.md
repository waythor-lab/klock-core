# klock-core

Rust coordination kernel for AI agents that mutate shared resources.

Klock stops the specific failure mode where multiple coding agents edit the same repo, all report success, the build still passes, and some intended work silently disappears. In our 5-agent Claude Code study, the solo baseline produced 3,027 intended changed lines; the concurrent run produced 2,311. About 716 intended lines, or 24%, disappeared while the build still passed.

`klock-core` is the deterministic engine underneath the Klock OSS workflow. It turns a proposed mutation into an explicit verdict before the write happens:

- `GRANT`: proceed with a time-bounded lease
- `WAIT`: hold and retry because an older agent can safely wait
- `DIE`: abort and retry later to prevent deadlock

## What Exists Today

- Rust coordination kernel with Wait-Die scheduling
- O(1) predicate conflict matrix
- in-memory lease store and optional SQLite-backed storage
- local HTTP server through `klock-cli`
- Python and JavaScript SDK surfaces through the same Rust-backed core
- LangChain tool wrapper for cooperative file-mutating tools
- deterministic proof scripts showing silent overwrite without Klock and coordinated behavior with Klock

OSS v1 coordinates cooperative agents that call Klock before mutating shared files. It is not transparent filesystem enforcement for arbitrary processes yet.

## Quick Proof

From the OSS repo root:

```bash
./scripts/klock_crash_test.sh
```

The proof shows:

- without Klock: two agents both report success, but one update disappears
- with Klock: both updates survive
- conflict timeline: `GRANT`, `WAIT`, and `DIE`
- LangChain path: the same protection through a real `BaseTool`

If you want the smaller manual path:

```bash
cd examples/oss_v1
python3 without_klock.py
python3 with_klock.py
python3 wait_die_trace.py
```

## Kernel Shape

```text
agent intent
    |
    v
Predicate + ResourceRef + agent priority
    |
    v
klock-core conflict matrix + Wait-Die scheduler
    |
    v
GRANT | WAIT | DIE
```

The kernel is deliberately small. It does not try to be an agent framework, workflow engine, or guardrail product. It answers one question before mutation: can this agent safely touch this resource right now?

## Rust Usage

```rust
use klock_core::client::KlockClient;

let mut klock = KlockClient::new();
klock.register_agent("refactor-bot", 100);

let result = klock.acquire_lease(
    "refactor-bot",
    "repo-run-001",
    "FILE",
    "src/auth.ts",
    "MUTATES",
    60_000,
);

if result.success {
    let lease_id = result.lease_id.expect("granted lease has id");
    // Mutate the file while the lease is held.
    klock.release_lease(&lease_id);
} else {
    // WAIT or DIE. Refresh context and retry according to your agent policy.
}
```

## Public Surfaces

- Website and docs: [klockcore.com](https://klockcore.com)
- npm package: [`@klock-protocol/core`](https://www.npmjs.com/package/@klock-protocol/core)
- Python package: [`klock`](https://pypi.org/project/klock/)

## License

MIT
