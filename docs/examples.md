# Examples

Practical scenarios demonstrating Klock's coordination in action.

---

## Example 1: Two Agents Editing the Same File

**Scenario**: A refactoring bot and a test-writing bot both want to modify `auth.ts`.

```python
from klock import KlockClient

klock = KlockClient()

# Register agents — refactor-bot is senior (started first)
klock.register_agent("refactor-bot", 100)
klock.register_agent("test-bot", 200)

# Senior acquires the file
result = klock.acquire_lease(
    "refactor-bot", "session-1",
    "FILE", "/src/auth.ts",
    "MUTATES", 30000  # 30s TTL
)
print(result)
# {'success': True, 'lease_id': '...', 'agent_id': 'refactor-bot', ...}

# Junior tries the same file → Wait-Die: DIE
conflict = klock.acquire_lease(
    "test-bot", "session-2",
    "FILE", "/src/auth.ts",
    "MUTATES", 30000
)
print(conflict)
# {'success': False, 'reason': 'DIE', 'wait_time': 1000}

# Junior works on a different file instead → OK
other = klock.acquire_lease(
    "test-bot", "session-2",
    "FILE", "/src/utils.ts",
    "MUTATES", 30000
)
print(other)
# {'success': True, 'lease_id': '...', ...}

# Senior finishes and releases
klock.release_lease(result["lease_id"])

# Now junior can acquire auth.ts
retry = klock.acquire_lease(
    "test-bot", "session-2",
    "FILE", "/src/auth.ts",
    "MUTATES", 30000
)
print(retry)
# {'success': True, 'lease_id': '...', ...}
```

---

## Example 2: Compatible Operations (Multiple Readers)

**Scenario**: Two agents both need to read a config file — this is safe.

```python
klock = KlockClient()
klock.register_agent("agent-a", 100)
klock.register_agent("agent-b", 200)

# Agent A reads the config
read_a = klock.acquire_lease(
    "agent-a", "s1", "CONFIG_KEY", "db.host", "CONSUMES", 60000
)
print(read_a["success"])  # True

# Agent B also reads the config → NO conflict (CONSUMES + CONSUMES is compatible)
read_b = klock.acquire_lease(
    "agent-b", "s2", "CONFIG_KEY", "db.host", "CONSUMES", 60000
)
print(read_b["success"])  # True — multiple readers are safe!

# But if Agent B tries to MUTATE while A is reading → CONFLICT
write_b = klock.acquire_lease(
    "agent-b", "s2", "CONFIG_KEY", "db.host", "MUTATES", 60000
)
print(write_b["success"])  # False — CONSUMES + MUTATES is incompatible
```

---

## Example 3: Multi-Resource Coordination via HTTP

**Scenario**: An agent needs to modify two files atomically. Using the REST API:

```bash
# Start the server
klock serve --port 3100

# Register the agent
curl -X POST http://localhost:3100/agents \
  -H 'Content-Type: application/json' \
  -d '{"agent_id": "migration-bot", "priority": 50}'

# Declare a multi-resource intent
curl -X POST http://localhost:3100/intents \
  -H 'Content-Type: application/json' \
  -d '{
    "agent_id": "migration-bot",
    "session_id": "migration-001",
    "intents": [
      {"resource_type": "DATABASE_TABLE", "resource_path": "users", "predicate": "MUTATES"},
      {"resource_type": "FILE", "resource_path": "/src/models/user.ts", "predicate": "MUTATES"}
    ]
  }'

# Response: {"status": "Granted", "conflicts": [], ...}

# Acquire individual leases for each resource
curl -X POST http://localhost:3100/leases \
  -H 'Content-Type: application/json' \
  -d '{
    "agent_id": "migration-bot", "session_id": "migration-001",
    "resource_type": "DATABASE_TABLE", "resource_path": "users",
    "predicate": "MUTATES", "ttl": 120000
  }'

curl -X POST http://localhost:3100/leases \
  -H 'Content-Type: application/json' \
  -d '{
    "agent_id": "migration-bot", "session_id": "migration-001",
    "resource_type": "FILE", "resource_path": "/src/models/user.ts",
    "predicate": "MUTATES", "ttl": 120000
  }'

# ... do migration work ...

# Release both leases
curl -X DELETE http://localhost:3100/leases/lease_migration-bot_...
```

---

## Example 4: Heartbeat to Extend a Long-Running Operation

**Scenario**: An agent is doing a long refactoring and needs to keep its lease alive.

```javascript
import { KlockClient } from '@klock-protocol/core';

const klock = new KlockClient();
klock.registerAgent('long-runner', 100);

const raw = klock.acquireLease(
  'long-runner', 'session-big',
  'FILE', '/src/monolith.ts',
  'MUTATES', 10000  // 10s TTL
);
const lease = JSON.parse(raw);

// Using HTTP heartbeat endpoint:
// POST http://localhost:3100/leases/{lease.leaseId}/heartbeat
// This resets the TTL timer, preventing expiration during long work.
```

---

## Example 5: Reentrant Access (Same Agent, Same Session)

**Scenario**: An agent acquires a lease, then needs it again in the same session.

```python
klock = KlockClient()
klock.register_agent("agent-x", 100)

# First lease on the file
first = klock.acquire_lease(
    "agent-x", "session-1",
    "FILE", "/src/app.ts",
    "MUTATES", 60000
)
print(first["success"])  # True

# Same agent, same session → reentrant, no conflict
second = klock.acquire_lease(
    "agent-x", "session-1",
    "FILE", "/src/app.ts",
    "MUTATES", 60000
)
print(second["success"])  # True — reentrant lock!
```
