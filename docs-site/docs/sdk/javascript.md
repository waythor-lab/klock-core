---
sidebar_position: 1
---

# JavaScript SDK

Native Node.js bindings for Klock, compiled from Rust via [napi-rs](https://napi.rs). Provides near-native performance for conflict detection and lease management.

## Installation

```bash
cd klock-js
pnpm install
pnpm run build        # Release build (optimized)
pnpm run build:debug  # Debug build (faster compilation)
```

After building, the native binary is available as `klock-core.darwin-arm64.node` (macOS ARM) or the equivalent for your platform.

## Package Info

- **NPM name**: `@klock-protocol/core`
- **Binary**: napi-rs compiled `.node` file
- **API surface**: Class-based, returns JSON strings

---

## API

### `new KlockClient()`

Creates a new coordination client with an empty state.

```javascript
import { KlockClient } from './index.js';
const klock = new KlockClient();
```

---

### `registerAgent(agentId: string, priority: number): void`

Register an agent with a priority. Lower values = older = higher precedence.

```javascript
klock.registerAgent('senior-bot', 100);
klock.registerAgent('junior-bot', 200);
```

---

### `acquireLease(agentId, sessionId, resourceType, resourcePath, predicate, ttl): string`

Attempt to acquire a lease. Returns a **JSON string** that must be parsed.

```javascript
const raw = klock.acquireLease(
  'senior-bot',     // agentId
  'session-1',      // sessionId
  'FILE',           // resourceType
  '/src/auth.ts',   // resourcePath
  'MUTATES',        // predicate
  60000             // ttl (ms)
);

const result = JSON.parse(raw);
```

**Success response:**
```json
{
  "success": true,
  "leaseId": "abc123",
  "agentId": "senior-bot",
  "resource": "FILE:/src/auth.ts",
  "expiresAt": 1708700060000
}
```

**Failure response (Wait-Die: Die):**
```json
{
  "success": false,
  "reason": "DIE",
  "waitTime": 1000
}
```

| Reason | Meaning |
|--------|---------|
| `DIE` | Junior agent — abort and retry later |
| `WAIT` | Senior agent — poll until holder releases |
| `CONFLICT` | General conflict |
| `RESOURCE_LOCKED` | Resource held by another operation |
| `SESSION_EXPIRED` | Session no longer valid |

---

### `releaseLease(leaseId: string): boolean`

Release a lease. Returns `true` if the lease was found and released.

```javascript
const released = klock.releaseLease(result.leaseId);
console.log(released); // true
```

---

### `activeLeaseCount(): number`

Get the count of currently active leases.

```javascript
console.log(klock.activeLeaseCount()); // 3
```

---

### `evictExpired(): number`

Remove expired leases. Returns the number evicted.

```javascript
const evicted = klock.evictExpired();
console.log(`Evicted ${evicted} expired leases`);
```

---

## Complete Example

```javascript
import { KlockClient } from './index.js';

const klock = new KlockClient();

// Register two agents with different priorities
klock.registerAgent('agent-senior', 100);  // Older = higher priority
klock.registerAgent('agent-junior', 200);  // Younger = lower priority

// Senior acquires a lease on auth.ts
const seniorLease = JSON.parse(
  klock.acquireLease('agent-senior', 's1', 'FILE', '/src/auth.ts', 'MUTATES', 60000)
);
console.log('Senior:', seniorLease);
// { success: true, leaseId: '...', ... }

// Junior tries the same file → DIE
const juniorResult = JSON.parse(
  klock.acquireLease('agent-junior', 's2', 'FILE', '/src/auth.ts', 'MUTATES', 60000)
);
console.log('Junior:', juniorResult);
// { success: false, reason: 'DIE', waitTime: 1000 }

// Junior works on a different file → OK
const juniorOther = JSON.parse(
  klock.acquireLease('agent-junior', 's2', 'FILE', '/src/utils.ts', 'MUTATES', 60000)
);
console.log('Junior (other file):', juniorOther);
// { success: true, leaseId: '...', ... }

// Cleanup
klock.releaseLease(seniorLease.leaseId);
klock.releaseLease(juniorOther.leaseId);
console.log('Active leases:', klock.activeLeaseCount()); // 0
```
