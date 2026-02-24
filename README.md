<div align="center">

# 🔒 Klock

**The Concurrency Control Plane for the Agent Economy**

[![CI](https://github.com/klock-protocol/klock/actions/workflows/ci.yml/badge.svg)](https://github.com/klock-protocol/klock/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/rust-1.75+-orange.svg)](https://www.rust-lang.org)

*Prevent the **Multi-Agent Race Condition (MARC)** — the silent data loss when autonomous AI agents simultaneously modify shared resources without coordination.*

[Getting Started](docs/getting-started.md) · [Architecture](docs/architecture.md) · [API Reference](docs/api-reference.md) · [Benchmarks](docs/benchmarks.md)

</div>

---

## The Problem

When multiple AI agents operate on the same codebase, they create **race conditions**:

```
Agent A: reads auth.ts → refactors login() → writes auth.ts
Agent B: reads auth.ts → adds 2FA to login() → writes auth.ts
                                                  ↑
                                          Agent A's changes: GONE
```

No crash. No error. The refactoring just silently vanishes. This is **MARC**.

## The Solution

Klock provides a **coordination kernel** that detects conflicts *before* they happen:

```python
from klock import KlockClient

klock = KlockClient()
klock.register_agent("refactor-bot", 100)  # Senior
klock.register_agent("2fa-bot", 200)       # Junior

# Senior acquires a lease
result = klock.acquire_lease("refactor-bot", "s1", "FILE", "/auth.ts", "MUTATES", 60000)
# ✅ {'success': True, 'lease_id': '...'}

# Junior tries the same file → blocked
conflict = klock.acquire_lease("2fa-bot", "s2", "FILE", "/auth.ts", "MUTATES", 60000)
# 🚫 {'success': False, 'reason': 'DIE', 'wait_time': 1000}
```

## Key Features

| Feature | Description |
|---------|-------------|
| ⚡ **O(1) Conflict Detection** | 6×6 predicate compatibility matrix — constant time regardless of active intents |
| 🛡️ **Deadlock-Free Scheduling** | Wait-Die protocol guarantees no circular waits |
| 🦀 **Rust Core** | Sub-nanosecond operations, zero GC, compiled to native bindings |
| 🐍 **Python SDK** | `from klock import KlockClient` via PyO3 |
| 📦 **JavaScript SDK** | `import { KlockClient } from '@klock-protocol/core'` via napi-rs |
| 🌐 **REST API** | `klock serve` — language-agnostic HTTP server |
| 💾 **Pluggable Storage** | In-memory (dev) or SQLite (persistent) backends |
| 🐳 **Docker Ready** | `docker compose up` — single command deployment |

## Quick Start

### Python
```bash
cd klock-py
pip install maturin && maturin develop --release
```
```python
from klock import KlockClient
klock = KlockClient()
klock.register_agent("my-agent", 100)
result = klock.acquire_lease("my-agent", "s1", "FILE", "/app.ts", "MUTATES", 60000)
```

### JavaScript
```bash
cd klock-js && pnpm install && pnpm run build
```
```javascript
import { KlockClient } from './index.js';
const klock = new KlockClient();
klock.registerAgent('my-agent', 100);
const result = JSON.parse(klock.acquireLease('my-agent', 's1', 'FILE', '/app.ts', 'MUTATES', 60000));
```

### HTTP Server
```bash
cargo build --release -p klock-cli
./target/release/klock serve --port 3100
```
```bash
# Register agent
curl -X POST http://localhost:3100/agents \
  -H 'Content-Type: application/json' \
  -d '{"agent_id": "bot-1", "priority": 100}'

# Acquire lease
curl -X POST http://localhost:3100/leases \
  -H 'Content-Type: application/json' \
  -d '{"agent_id":"bot-1","session_id":"s1","resource_type":"FILE","resource_path":"/app.ts","predicate":"MUTATES","ttl":60000}'
```

### Docker
```bash
docker compose up -d
curl http://localhost:3100/health
```

## Architecture

```
┌────────────────────────────────────────────────────┐
│                  Klock Kernel (Rust)                │
│                                                    │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────┐ │
│  │   Conflict    │  │  Wait-Die    │  │  Lease   │ │
│  │   Engine      │→│  Scheduler   │→│  Store   │ │
│  │   O(1) matrix │  │  deadlock    │  │  memory  │ │
│  │              │  │  prevention  │  │  sqlite  │ │
│  └──────────────┘  └──────────────┘  └──────────┘ │
└──────────┬─────────────────┬──────────────┬────────┘
           │                 │              │
    ┌──────┴──────┐   ┌──────┴──────┐  ┌───┴────┐
    │  Python SDK │   │   JS SDK    │  │  HTTP  │
    │  (PyO3)     │   │  (napi-rs)  │  │ Server │
    └─────────────┘   └─────────────┘  └────────┘
```

## Performance

All benchmarks measured on Apple Silicon in release mode:

| Operation | Latency | Complexity |
|-----------|---------|------------|
| Conflict check (single pair) | **~1 ns** | O(1) |
| Conflict check (1000 triples) | **~339 ns** | O(1) ✓ |
| Wait-Die decision | **~25 ns** | O(n) leases |
| Full kernel execute | **~500 ns** | O(1) |
| Lease acquire + release | **~670 ns** | O(n) leases |

> **O(1) conflict detection confirmed**: 10, 100, and 1000 active triples all resolve in ~337ns.

See [benchmarks.md](docs/benchmarks.md) for full details.

## Project Structure

```
Klock-OpenSource/
├── klock-core/          # Rust coordination kernel
│   ├── src/conflict.rs  # 6×6 compatibility matrix
│   ├── src/scheduler.rs # Wait-Die deadlock prevention
│   ├── src/client.rs    # High-level API
│   └── src/infrastructure_sqlite.rs  # Persistent storage
├── klock-js/            # Node.js native bindings (napi-rs)
├── klock-py/            # Python native bindings (PyO3)
├── klock-cli/           # CLI + Axum HTTP server
├── docs/                # Documentation
├── docs-site/           # Docusaurus documentation site
├── Dockerfile           # Multi-stage container build
└── docker-compose.yml   # One-command deployment
```

## Server Configuration

| Flag | Default | Env Var | Description |
|------|---------|---------|-------------|
| `--port` | `3100` | — | HTTP port |
| `--host` | `0.0.0.0` | — | Bind address |
| `--storage` | `memory` | `KLOCK_STORAGE` | `memory` or `sqlite:<path>` |
| — | — | `KLOCK_API_KEY` | API key for authentication (optional) |

```bash
# Persistent storage with auth
KLOCK_API_KEY=my-secret-key klock serve --storage sqlite:./klock.db
```

## The Klock Protocol (KLIS)

Klock implements the **KLIS** (Klock Intent Serialization) protocol:

- **KLIS-0**: SPO Triple Model — every intent is (Subject, Predicate, Object)
- **KLIS-2**: 6×6 Conflict Matrix — O(1) compatibility lookup
- **KLIS-3**: Wait-Die Protocol — deadlock-free priority scheduling
- **KLIS-4**: Lease Lifecycle — Active → Released/Expired/Revoked

See [protocol-spec.md](docs/protocol-spec.md) for the full specification.

## Documentation

| Document | Description |
|----------|-------------|
| [Getting Started](docs/getting-started.md) | Install and run in 5 minutes |
| [Architecture](docs/architecture.md) | Core concepts and design |
| [API Reference](docs/api-reference.md) | REST API endpoints |
| [JavaScript SDK](docs/sdk-javascript.md) | Node.js bindings |
| [Python SDK](docs/sdk-python.md) | Python bindings |
| [Protocol Spec](docs/protocol-spec.md) | KLIS formal specification |
| [Examples](docs/examples.md) | 5 worked scenarios |
| [Benchmarks](docs/benchmarks.md) | Performance data |
| [Docker](docs/docker.md) | Container deployment |

## License

MIT — See [LICENSE](LICENSE) for details.

---

<div align="center">
<sub>Built with 🦀 Rust · Designed for the Agent Economy</sub>
</div>
