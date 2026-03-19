# klock

Python SDK for Klock OSS v1.

The package now exposes two entrypoints:

- `KlockClient` for embedded local coordination
- `KlockHttpClient` for talking to `klock-cli serve`

## Install

```bash
pip install klock
```

## Embedded client

```python
from klock import KlockClient

klock = KlockClient()
klock.register_agent("agent-a", 100)
result = klock.acquire_lease("agent-a", "session-a", "FILE", "/src/auth.js", "MUTATES", 5000)
```

## HTTP client

```python
from klock import KlockHttpClient

klock = KlockHttpClient("http://localhost:3100")
klock.register_agent("agent-a", 100)
result = klock.acquire_lease("agent-a", "session-a", "FILE", "/src/auth.js", "MUTATES", 5000)
```

Use `KlockHttpClient` for the OSS v1 local repo/workspace coordination workflow.
