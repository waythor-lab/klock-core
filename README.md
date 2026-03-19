<div align="center">

<img src="docs/assets/logo.svg" alt="Klock Logo" width="120" />
<br />
<img src="docs/assets/logo-text.svg" alt="Klock Logo Text" width="180" />

# Klock OSS v1

Coordinate AI coding agents before they overwrite each other in the same repo.

[Getting Started](docs/getting-started.md) · [LangChain](integrations/klock-langchain/README.md) · [Examples](docs/examples.md) · [Benchmarks](docs/benchmarks.md)

</div>

## What ships in OSS v1

- `klock-core`: Rust coordination engine with Wait-Die scheduling
- `klock-py`: embedded and HTTP-backed Python client
- `klock-js`: embedded and HTTP-backed JavaScript client
- `klock-langchain`: canonical integration for cooperative file-mutating tools
- `klock-cli`: local coordination server for shared repo/workspace access
- deterministic proof demos that show silent overwrite without Klock and a conflict-managed Wait-Die outcome with Klock

## The product in one sentence

Multiple coding agents can target the same repo. Klock makes cooperative agents acquire leases before mutating shared files, so conflicting edits become visible `GRANT`, `WAIT`, and `DIE` decisions instead of silent lost work.

OSS v1 is coordination for compliant actors. It is not yet a filesystem-enforced layer.

## Quick Proof

Install the Python packages:

```bash
pip install klock klock-langchain
```

Reproduce the failure:

```bash
cd examples/oss_v1
python3 without_klock.py
```

Run the full proof:

```bash
./scripts/run_oss_v1_demo.sh
```

Expected outcome:

- `without_klock.py`: both agents succeed, one feature vanishes
- `with_klock.py`: both features survive
- `wait_die_trace.py`: explicit `GRANT`, `WAIT`, and `DIE` output on the same file
- localhost clients auto-start the local `klock` server when the binary or source-tree command is available
- auto-start logs the base URL and PID, and you can disable it with `KLOCK_DISABLE_AUTOSTART=1`

One-command version:

```bash
./scripts/run_oss_v1_demo.sh
```

## Canonical LangChain example

```python
from klock import KlockHttpClient
from klock_langchain import klock_protected
from langchain_core.tools import BaseTool

klock = KlockHttpClient(base_url="http://localhost:3100")
klock.register_agent("refactor-bot", 100)

class WriteFileTool(BaseTool):
    name = "write_file"
    description = "Mutates a repo file after acquiring a Klock lease."

    @klock_protected(
        klock_client=klock,
        agent_id="refactor-bot",
        session_id="repo-run-001",
        resource_type="FILE",
        resource_path_extractor=lambda kwargs: kwargs["path"],
        predicate="MUTATES",
    )
    def _run(self, path: str, content: str) -> str:
        with open(path, "w", encoding="utf-8") as handle:
            handle.write(content)
        return f"updated {path}"
```

Why this is the v1 path:

- it works with the local Klock server today
- it coordinates real file-mutating tools
- it maps directly to the repo coordination proof scripts
- it gives users one integration to learn first

## SDK surface

### Python

```bash
pip install klock
```

```python
from klock import KlockClient, KlockHttpClient

embedded = KlockClient()
remote = KlockHttpClient("http://localhost:3100")
```

### JavaScript

```bash
npm install @klock-protocol/core
```

```javascript
const { KlockClient, KlockHttpClient } = require('@klock-protocol/core');

const embedded = new KlockClient();
const remote = new KlockHttpClient({ baseUrl: 'http://localhost:3100' });
```

## Local repo workflow

1. Let `KlockHttpClient` auto-start `klock-cli serve`, or start it manually.
2. Register each agent with an explicit priority.
3. Wrap every file-mutating tool with `klock_protected(...)` or call the SDK directly.
4. Acquire a lease before reading and writing the target file.
5. Release the lease when the mutation is complete.
6. Retry on `WAIT` or `DIE` according to your agent policy.

## Documentation

- [Getting Started](docs/getting-started.md)
- [Examples](docs/examples.md)
- [Python SDK](docs/sdk-python.md)
- [JavaScript SDK](docs/sdk-javascript.md)
- [Benchmarks and Proof](docs/benchmarks.md)
- [LangChain integration](integrations/klock-langchain/README.md)
- [Agent support](docs/agent-support.md)
- [Releasing OSS v1](docs/releasing.md)

## Not the OSS v1 front door

The dashboard, cluster, sidecar, policy engine, and Kubernetes packaging are not the public product surface for this release. This repo is centered on local multi-agent repo coordination first.
