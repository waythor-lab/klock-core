# Architecture

Klock's architecture follows a **pure kernel + infrastructure** separation where the coordination logic is entirely deterministic and free of side effects.

## Design Principles

1. **Pure Kernel**: The conflict engine, scheduler, and state machine have zero I/O — they are pure functions over data
2. **Constant-time pairwise compatibility**: A precomputed 6×6 predicate compatibility matrix means each held-vs-requested pair is decided in O(1). The full conflict scan is O(n) in the number of currently held leases (the matrix lookup is the inner loop).
3. **Deadlock Freedom**: Wait-Die scheduling produces a strict ordering on agents — older waits, younger dies — so no circular waits can form. Equal priorities are tie-broken deterministically by `agent_id` to preserve antisymmetry.
4. **Kernel-Centered Correctness**: Within the kernel, correctness is enforced rather than requested. In OSS v1, agents still need to call Klock before mutating shared resources.

---

## Module Map

```
klock-core/
├── types/           # Predicate, ResourceRef, SPOTriple, Lease
├── conflict.rs      # O(1) conflict detection engine
├── scheduler.rs     # Wait-Die deadlock prevention
├── state.rs         # KlockKernel::execute() — main entry point
├── infrastructure.rs         # LeaseStore trait
├── infrastructure_in_memory.rs  # In-memory implementation
└── client.rs        # KlockClient — high-level API
```

---

## Core Concepts

### 1. SPO Triples (Subject-Predicate-Object)

Every agent intent is expressed as a triple:

| Part | Description | Example |
|------|-------------|---------|
| **Subject** | Agent ID | `"refactor-bot"` |
| **Predicate** | Operation type | `Mutates` |
| **Object** | Target resource | `FILE:/src/auth.ts` |

### 2. Predicates (6 operation types)

| Predicate | Meaning | Example |
|-----------|---------|---------|
| `Provides` | Creates/exports something new | Adding a new function |
| `Consumes` | Reads/imports existing | Reading a config file |
| `Mutates` | Modifies existing | Editing a function body |
| `Deletes` | Removes existing | Removing a file |
| `DependsOn` | Requires existence | Importing a module |
| `Renames` | Renames a resource | Renaming a file |

### 3. The 6×6 Conflict Matrix

The conflict engine uses a constant-time matrix lookup to determine if two predicates are compatible:

```
            Provides  Consumes  Mutates  Deletes  DependsOn  Renames
Provides    ✗CONF     ✓OK       ✗CONF    ✗CONF    ✓OK        ✗CONF
Consumes    ✓OK       ✓OK       ✗CONF    ✗CONF    ✓OK        ✗CONF
Mutates     ✗CONF     ✗CONF     ✗CONF    ✗CONF    ✗CONF      ✗CONF
Deletes     ✗CONF     ✗CONF     ✗CONF    ✗CONF    ✗CONF      ✗CONF
DependsOn   ✓OK       ✓OK       ✗CONF    ✗CONF    ✓OK        ✗CONF
Renames     ✗CONF     ✗CONF     ✗CONF    ✗CONF    ✗CONF      ✗CONF
```

**Key rules:**
- `Mutates`, `Deletes`, and `Renames` conflict with everything
- `Consumes`/`DependsOn` are compatible with each other (multiple readers OK)
- `Provides` conflicts with another `Provides` (two agents creating the same thing)
- Same agent + same session = no conflict (reentrant lock)

---

## Wait-Die Protocol

When a conflict is detected, the **Wait-Die** scheduler resolves it using agent priorities:

```
Agent Priority = Registration Timestamp (lower = older = higher priority)

IF requester.priority < holder.priority:
    → WAIT (older agent waits for younger to finish)
ELSE:
    → DIE  (younger agent aborts and retries later)
```

### Why Wait-Die?

| Property | Guarantee |
|----------|-----------|
| **Deadlock freedom** | Waiting edges only flow old→young, so cycles are impossible |
| **Liveness** | The oldest agent always makes progress |
| **Bounded retry** | Younger agents eventually become oldest and proceed |
| **No starvation** | Priority is stable (timestamp never changes) |

### The Three Verdicts

| Verdict | Meaning | Agent Action |
|---------|---------|--------------|
| `Granted` | No conflict — proceed | Execute intent |
| `Wait` | Conflict exists, but you're senior — hold | Poll until holder releases |
| `Die` | Conflict exists, and you're junior — abort | Retry with backoff |

---

## Execution Flow

```
┌─────────────┐     ┌──────────────────┐     ┌────────────────┐
│ IntentManifest │──→│ ConflictEngine   │──→│ WaitDieScheduler│
│ (agent + intents)│ │ check() O(1)     │   │ decide()        │
└─────────────┘     └──────────────────┘     └────────────────┘
                          │                         │
                          ↓                         ↓
                    No Conflict?              Conflict Found?
                    → Granted                 → Wait or Die
```

1. Agent submits an `IntentManifest` (session + list of SPO triples)
2. `ConflictEngine::check()` tests each triple against active intents
3. If conflict found → `WaitDieScheduler::decide()` compares priorities
4. `KlockKernel::execute()` returns worst-case `KernelVerdict`

---

## Resource Types

| Type | Key Format | Example |
|------|------------|---------|
| `File` | `FILE:/path/to/file` | `FILE:/src/auth.ts` |
| `Symbol` | `SYMBOL:ClassName.method` | `SYMBOL:User.authenticate` |
| `ApiEndpoint` | `API_ENDPOINT:/route` | `API_ENDPOINT:/api/users` |
| `DatabaseTable` | `DATABASE_TABLE:name` | `DATABASE_TABLE:users` |
| `ConfigKey` | `CONFIG_KEY:key` | `CONFIG_KEY:db.host` |

---

## Lease Lifecycle

```
  acquire()        heartbeat()        release()
     │                 │                  │
     ▼                 ▼                  ▼
  ┌──────┐         ┌──────┐          ┌──────────┐
  │Active│────────→│Active│─────────→│ Released  │
  └──────┘  renew  └──────┘  explicit └──────────┘
     │                                    
     │  TTL expires                       
     ▼                                    
  ┌──────────┐                            
  │ Expired  │ ← evict_expired()          
  └──────────┘                            
```

- **Active**: Lease is held and valid
- **Expired**: TTL elapsed without heartbeat
- **Released**: Explicitly freed by the agent
- **Revoked**: Forcibly cancelled (conflict resolution)

---

## The Klock Contract

> **Safety (Intent Isolation)**: At any time, no two simultaneously granted leases contain conflicting intents on the same resource.

> **Liveness (Deadlock Freedom)**: Waiting edges in the Wait-Die protocol only flow from older to younger agents along a strict order on `(priority, agent_id)`. Cycles are impossible. Progress is bounded by the youngest agent eventually retrying after backoff or by lease expiry under TTL.

> **Intent Serializability**: Completed executions admit a serial order consistent with the conflict-induced partial order.
