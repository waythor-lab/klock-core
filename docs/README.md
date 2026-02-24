# Klock Documentation

> **The Concurrency Control Plane for the Agent Economy**

Klock prevents the **Multi-Agent Race Condition (MARC)** — the silent data loss that occurs when autonomous AI agents simultaneously modify shared resources without coordination.

## Documentation Index

| Document | Description |
|----------|-------------|
| [Getting Started](./getting-started.md) | Install Klock and run your first conflict check |
| [Architecture](./architecture.md) | Core concepts: conflict engine, Wait-Die scheduling, intent manifests |
| [REST API Reference](./api-reference.md) | HTTP endpoints for `klock serve` |
| [JavaScript SDK](./sdk-javascript.md) | Native Node.js bindings via napi-rs |
| [Python SDK](./sdk-python.md) | Native Python bindings via PyO3 |
| [Benchmarks](./benchmarks.md) | Performance results and methodology |
| [Docker Deployment](./docker.md) | Container deployment guide |

## Quick Example

```python
from klock import KlockClient

klock = KlockClient()
klock.register_agent("refactor-bot", 100)

result = klock.acquire_lease(
    "refactor-bot", "session-1",
    "FILE", "/src/auth.ts",
    "MUTATES", 60000
)

if result["success"]:
    print(f"✅ Lease acquired: {result['lease_id']}")
    # ... agent does work ...
    klock.release_lease(result["lease_id"])
else:
    print(f"🚫 Blocked: {result['reason']}")
```

## Project Structure

```
Klock-OpenSource/
├── klock-core/     # Rust coordination kernel (MIT)
├── klock-js/       # Node.js native bindings
├── klock-py/       # Python native bindings
├── klock-cli/      # CLI + HTTP server
├── Dockerfile      # Multi-stage container build
└── docs/           # This documentation
```

## License

MIT — See [OPEN_CORE.md](../../Strategy/OPEN_CORE.md) for the full open-core philosophy.
