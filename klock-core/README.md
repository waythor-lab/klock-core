# klock-core

> The deterministic coordination kernel for the Klock protocol.

**klock-core** is the pure, side-effect-free engine that powers Klock's intent-based concurrency control for multi-agent systems. It prevents the **Multi-Agent Race Condition (MARC)** — silent data corruption that occurs when autonomous AI agents simultaneously modify shared resources.

## Architecture

```
┌─────────────────────────────────────────────────┐
│                  klock-core                      │
│                                                  │
│  ┌──────────┐  ┌────────────┐  ┌──────────────┐ │
│  │  types   │  │  conflict  │  │  scheduler   │ │
│  │          │  │  (O(1)     │  │  (Wait-Die   │ │
│  │ SPOTriple│  │   Matrix)  │  │   Protocol)  │ │
│  │ Lease    │  │            │  │              │ │
│  │ Predicate│  │            │  │              │ │
│  └──────────┘  └────────────┘  └──────────────┘ │
│                                                  │
│  ┌──────────────────────────────────────────────┐│
│  │              state (KlockKernel)             ││
│  │   Orchestrates conflict + scheduling         ││
│  └──────────────────────────────────────────────┘│
│                                                  │
│  ┌──────────────────────────────────────────────┐│
│  │       infrastructure (LeaseStore trait)       ││
│  │   InMemoryLeaseStore (reference impl)        ││
│  └──────────────────────────────────────────────┘│
└─────────────────────────────────────────────────┘
```

## Modules

| Module | Purpose |
|--------|---------|
| `types` | Core protocol primitives: `Predicate`, `ResourceRef`, `SPOTriple`, `Lease` |
| `conflict` | O(1) conflict detection via precomputed 6×6 compatibility matrix |
| `scheduler` | Wait-Die deadlock prevention protocol |
| `state` | `KlockKernel::execute()` — the deterministic core orchestrator |
| `infrastructure` | `LeaseStore` trait + `InMemoryLeaseStore` reference implementation |

## Usage

```rust
use klock_core::state::{KlockKernel, IntentManifest, StateSnapshot};
use klock_core::types::*;

let state = StateSnapshot { /* ... */ };
let manifest = IntentManifest { /* ... */ };

let verdict = KlockKernel::execute(&state, &manifest);
match verdict.status {
    KernelVerdictStatus::Granted => println!("Safe to proceed"),
    KernelVerdictStatus::Wait    => println!("Wait for senior agent"),
    KernelVerdictStatus::Die     => println!("Abort and retry"),
}
```

## License

MIT
