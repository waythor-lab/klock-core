# Python SDK

Native Python bindings for Klock, compiled from Rust via [PyO3](https://pyo3.rs) and [maturin](https://www.maturin.rs). Provides near-native performance with a Pythonic dict-based API.

## Installation

```bash
cd klock-py
python -m venv .venv && source .venv/bin/activate
pip install maturin
maturin develop --release
```

## Package Info

- **PyPI name**: `klock`
- **Package**: `klock`
- **ABI**: ABI3 (compatible across Python 3.8+)
- **API surface**: Class-based, returns Python dicts

---

## API

### `KlockClient()`

Creates a new coordination client with an empty state.

```python
from klock import KlockClient

klock = KlockClient()
```

---

### `register_agent(agent_id: str, priority: int) -> None`

Register an agent with a priority. Lower values = older = higher precedence.

```python
klock.register_agent("senior-bot", 100)
klock.register_agent("junior-bot", 200)
```

---

### `acquire_lease(agent_id, session_id, resource_type, resource_path, predicate, ttl) -> dict`

Attempt to acquire a lease. Returns a Python dict directly.

```python
result = klock.acquire_lease(
    "senior-bot",      # agent_id
    "session-1",       # session_id
    "FILE",            # resource_type
    "/src/auth.ts",    # resource_path
    "MUTATES",         # predicate
    60000              # ttl (ms)
)
```

**Success result:**
```python
{
    "success": True,
    "lease_id": "abc123",
    "agent_id": "senior-bot",
    "resource": "FILE:/src/auth.ts",
    "expires_at": 1708700060000
}
```

**Failure result (Wait-Die: Die):**
```python
{
    "success": False,
    "reason": "DIE",
    "wait_time": 1000
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

### `release_lease(lease_id: str) -> bool`

Release a lease. Returns `True` if the lease was found and released.

```python
released = klock.release_lease(result["lease_id"])
assert released is True
```

---

### `active_lease_count() -> int`

Get the count of currently active leases.

```python
print(klock.active_lease_count())  # 3
```

---

### `evict_expired() -> int`

Remove expired leases. Returns the number evicted.

```python
evicted = klock.evict_expired()
print(f"Evicted {evicted} expired leases")
```

---

## Complete Example

```python
from klock import KlockClient

klock = KlockClient()

# Register two agents
klock.register_agent("agent-senior", 100)  # Older = higher priority
klock.register_agent("agent-junior", 200)  # Younger = lower priority

# Senior acquires a lease on auth.ts
senior_result = klock.acquire_lease(
    "agent-senior", "s1", "FILE", "/src/auth.ts", "MUTATES", 60000
)
print(f"Senior: {senior_result}")
# {'success': True, 'lease_id': '...', 'agent_id': 'agent-senior', ...}

# Junior tries the same file → DIE
junior_result = klock.acquire_lease(
    "agent-junior", "s2", "FILE", "/src/auth.ts", "MUTATES", 60000
)
print(f"Junior: {junior_result}")
# {'success': False, 'reason': 'DIE', 'wait_time': 1000}

# Junior can work on a different file → OK
junior_other = klock.acquire_lease(
    "agent-junior", "s2", "FILE", "/src/utils.ts", "MUTATES", 60000
)
print(f"Junior (other file): {junior_other}")
# {'success': True, 'lease_id': '...', ...}

# Cleanup
klock.release_lease(senior_result["lease_id"])
klock.release_lease(junior_other["lease_id"])
print(f"Active leases: {klock.active_lease_count()}")  # 0
```

---

## Wait-Die Protocol in Action

```python
import time

klock = KlockClient()
klock.register_agent("senior", 100)
klock.register_agent("junior", 200)

# Senior holds a lock
klock.acquire_lease("senior", "s1", "FILE", "/db.ts", "MUTATES", 5000)

# Junior tries and gets DIE
result = klock.acquire_lease("junior", "s2", "FILE", "/db.ts", "MUTATES", 5000)
assert result["success"] is False
assert result["reason"] == "DIE"

# Wait for senior's TTL to expire, then evict
time.sleep(6)
klock.evict_expired()

# Junior retries → success
retry = klock.acquire_lease("junior", "s2", "FILE", "/db.ts", "MUTATES", 5000)
assert retry["success"] is True
print("✅ Junior acquired after senior's lease expired")
```
