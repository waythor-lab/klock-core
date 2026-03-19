# Python SDK

The Python SDK ships two client surfaces.

## `KlockClient`

Use this for embedded, in-process coordination.

```python
from klock import KlockClient

klock = KlockClient()
klock.register_agent("agent-a", 100)
result = klock.acquire_lease("agent-a", "session-a", "FILE", "/src/auth.js", "MUTATES", 5000)
```

## `KlockHttpClient`

Use this for the OSS v1 local-server workflow.

```python
from klock import KlockHttpClient

klock = KlockHttpClient("http://localhost:3100")
klock.register_agent("agent-a", 100)
result = klock.acquire_lease("agent-a", "session-a", "FILE", "/src/auth.js", "MUTATES", 5000)
```

When the client targets `localhost`, it auto-starts the local server by default using:

1. `KLOCK_SERVER_COMMAND`
2. installed `klock` binary
3. source-tree `cargo run --release -p klock-cli -- serve`

When auto-start happens, the SDK logs:

- the base URL
- the launch command
- the PID of the spawned server

Disable auto-start with either:

- `KLOCK_DISABLE_AUTOSTART=1`
- `KlockHttpClient(..., auto_start=False)`

### Available methods

- `register_agent(agent_id, priority)`
- `acquire_lease(agent_id, session_id, resource_type, resource_path, predicate, ttl)`
- `release_lease(lease_id)`
- `heartbeat_lease(lease_id)`
- `list_leases()`
- `auto_start_enabled()`
- `auto_start_disabled_by_env()`
- `last_started_pid()`

## Recommended pairing

For LangChain, pair `KlockHttpClient` with `klock-langchain`:

```python
from klock import KlockHttpClient
from klock_langchain import klock_protected
```
