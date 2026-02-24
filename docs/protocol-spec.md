# Protocol Specification

Klock implements the **KLIS** (Klock Intent Serialization) protocol. This specification defines the core primitives and their interactions.

---

## KLIS-0: SPO Triple Model

Every agent intent is expressed as a **Subject-Predicate-Object (SPO)** triple:

```
Triple := (Subject, Predicate, Object, Confidence, Timestamp)
```

| Field | Type | Description |
|-------|------|-------------|
| `Subject` | `AgentID` | The agent declaring the intent |
| `Predicate` | `Predicate` | The operation type |
| `Object` | `ResourceRef` | The target resource |
| `Confidence` | `High \| Medium \| Low` | Inference confidence |
| `Timestamp` | `u64` (ms) | When the intent was registered |

---

## KLIS-1: Predicate Taxonomy

Six predicates describe all agent-resource interactions:

| Predicate | Semantics | Example |
|-----------|-----------|---------|
| `PROVIDES` | Agent creates or exports a new artifact | Adding a new function |
| `CONSUMES` | Agent reads or imports existing artifact | Reading config |
| `MUTATES` | Agent modifies existing artifact in-place | Editing function body |
| `DELETES` | Agent removes an existing artifact | Deleting a file |
| `DEPENDS_ON` | Agent requires artifact to exist | Importing a module |
| `RENAMES` | Agent renames an artifact | Renaming a file |

---

## KLIS-2: Conflict Compatibility Matrix

The kernel uses a **6×6 boolean matrix** for O(1) conflict detection:

```
COMPAT[i][j] = true iff Predicate_i and Predicate_j can coexist on the same resource
```

```
          PRO  CON  MUT  DEL  DEP  REN
PROVIDES [ F    T    F    F    T    F  ]
CONSUMES [ T    T    F    F    T    F  ]
MUTATES  [ F    F    F    F    F    F  ]
DELETES  [ F    F    F    F    F    F  ]
DEPENDS  [ T    T    F    F    T    F  ]
RENAMES  [ F    F    F    F    F    F  ]
```

**Key invariant**: `COMPAT[i][j] == COMPAT[j][i]` (symmetric matrix)

---

## KLIS-3: Wait-Die Scheduling Protocol

When a conflict is detected between two agents, the **Wait-Die** protocol resolves it:

```
GIVEN:
  requester: Agent requesting a lease
  holder:    Agent currently holding a conflicting lease
  
IF requester.priority < holder.priority:
  # Requester is OLDER (lower timestamp = earlier registration)
  → VERDICT: WAIT
  # Requester waits for holder to release
  
ELSE:
  # Requester is YOUNGER (higher timestamp = later registration)
  → VERDICT: DIE
  # Requester must abort and retry with exponential backoff
```

**Properties**:
- **Deadlock-free**: Waiting edges only flow old → young (no cycles possible)
- **Starvation-free**: An agent's priority never changes, so it eventually becomes the oldest
- **Liveness**: The oldest agent in any conflict set always makes progress

---

## KLIS-4: Lease Lifecycle

```
States: { Active, Expired, Released, Revoked }

Transitions:
  ACQUIRE  → Active
  HEARTBEAT → Active (renew TTL)
  RELEASE  → Released
  TTL_EXPIRY → Expired
  FORCE_REVOKE → Revoked
```

| Transition | Trigger | Side Effect |
|------------|---------|-------------|
| `ACQUIRE` | Agent requests lease, no conflict | Creates Active lease |
| `HEARTBEAT` | Agent sends keepalive | Resets `expires_at = now + ttl` |
| `RELEASE` | Agent explicitly frees lease | Marks Released |
| `TTL_EXPIRY` | `now > expires_at` | Marks Expired on next eviction |
| `FORCE_REVOKE` | Admin or conflict resolution | Marks Revoked (enterprise) |

---

## KLIS-5: Kernel Execution Pipeline

```
IntentManifest → ConflictEngine → WaitDieScheduler → KernelVerdict

Input:  IntentManifest { agent_id, session_id, intents: [SPOTriple] }
Output: KernelVerdict  { status: Granted|Wait|Die, conflicts: [String] }
```

**Execution steps**:

1. For each intent in the manifest:
   a. `ConflictEngine::check(intent, active_intents)` → O(1) matrix lookup
   b. If conflict found: `WaitDieScheduler::decide(requester, holder)` → Wait or Die
2. Return worst-case verdict across all intents:
   - Any `Die` → entire manifest gets `Die`
   - Any `Wait` (no Die) → entire manifest gets `Wait`
   - All clear → `Granted`

---

## KLIS-6: Resource Addressing

Resources are identified by a `(ResourceType, Path)` tuple:

```
ResourceKey := "{ResourceType}:{Path}"
```

| ResourceType | Key Example |
|-------------|-------------|
| `FILE` | `FILE:/src/auth.ts` |
| `SYMBOL` | `SYMBOL:User.authenticate` |
| `API_ENDPOINT` | `API_ENDPOINT:/api/users` |
| `DATABASE_TABLE` | `DATABASE_TABLE:users` |
| `CONFIG_KEY` | `CONFIG_KEY:db.host` |

---

## KLIS-7: Reentrant Lock Semantics

Same-agent, same-session intents do **not** conflict with each other:

```
IF triple_a.subject == triple_b.subject 
   AND triple_a.session_id == triple_b.session_id:
  → NO CONFLICT (reentrant)
```

This allows an agent to acquire multiple leases on the same resource within a single session.
