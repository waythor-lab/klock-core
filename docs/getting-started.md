# Getting Started

Get Klock running in under 5 minutes.

## Prerequisites

- **Rust** 1.75+ (for building from source)
- **Node.js** 18+ (for JavaScript bindings)
- **Python** 3.8+ (for Python bindings)

---

## Option 1: Python SDK

### Install

```bash
cd klock-py
python -m venv .venv && source .venv/bin/activate
pip install maturin
maturin develop --release
```

### Use

```python
from klock import KlockClient

klock = KlockClient()

# 1. Register agents (lower priority = older = higher precedence)
klock.register_agent("agent-senior", 100)
klock.register_agent("agent-junior", 200)

# 2. Senior acquires a lease
result = klock.acquire_lease(
    "agent-senior", "session-1",
    "FILE", "/src/app.ts",
    "MUTATES", 60000  # 60s TTL
)
print(result)
# {'success': True, 'lease_id': '...', 'agent_id': 'agent-senior', ...}

# 3. Junior attempts the same resource → blocked by Wait-Die
conflict = klock.acquire_lease(
    "agent-junior", "session-2",
    "FILE", "/src/app.ts",
    "MUTATES", 60000
)
print(conflict)
# {'success': False, 'reason': 'DIE', 'wait_time': 1000}

# 4. Release when done
klock.release_lease(result["lease_id"])
```

---

## Option 2: JavaScript SDK

### Install

```bash
cd klock-js
pnpm install
pnpm run build
```

### Use

```javascript
import { KlockClient } from './index.js';

const klock = new KlockClient();

klock.registerAgent('agent-1', 100);

const raw = klock.acquireLease(
  'agent-1', 'session-1',
  'FILE', '/src/app.ts',
  'MUTATES', 60000
);
const result = JSON.parse(raw);

if (result.success) {
  console.log(`Lease acquired: ${result.leaseId}`);
  // ... do work ...
  klock.releaseLease(result.leaseId);
}
```

---

## Option 3: HTTP Server (Language-Agnostic)

### Build & Run

```bash
cd Klock-OpenSource
cargo build --release -p klock-cli
./target/release/klock serve --port 3100
```

### Use via curl

```bash
# Health check
curl http://localhost:3100/health

# Register an agent
curl -X POST http://localhost:3100/agents \
  -H 'Content-Type: application/json' \
  -d '{"agent_id": "bot-1", "priority": 100}'

# Acquire a lease
curl -X POST http://localhost:3100/leases \
  -H 'Content-Type: application/json' \
  -d '{
    "agent_id": "bot-1",
    "session_id": "s1",
    "resource_type": "FILE",
    "resource_path": "/src/auth.ts",
    "predicate": "MUTATES",
    "ttl": 60000
  }'

# Release a lease
curl -X DELETE http://localhost:3100/leases/<lease-id>
```

---

## Option 4: Docker

```bash
docker build -t klock-server .
docker run -p 3100:3100 klock-server
```

---

## Next Steps

- Read the [Architecture](./architecture.md) doc to understand Wait-Die and the conflict matrix
- See the [API Reference](./api-reference.md) for all server endpoints
- Check [Benchmarks](./benchmarks.md) for performance characteristics
