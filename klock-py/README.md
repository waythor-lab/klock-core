# klock

[![PyPi version](https://badge.fury.io/py/klock.svg)](https://pypi.org/project/klock/)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

**The Coordination Kernel for the Agent Economy.**

`klock` is a high-performance, Rust-powered distributed coordination engine designed specifically to solve the **Multi-Agent Race Condition (MARC)**. 

When multiple autonomous AI agents (like Claude Code, SWE-agent, or custom swarms) edit a shared codebase or resource simultaneously, they often overwrite each other's work without warning. `klock` prevents this by treating resource access as a first-class scheduled event using the **Wait-Die** protocol.

## Features

- **Deadlock Immunity**: Uses timestamp-ordered priority scheduling (Wait-Die) to ensure global progress without cycles.
- **High Performance**: Core engine written in Rust for sub-millisecond lock acquisition.
- **Agent Awareness**: Designed for autonomous systems that can "Retry" or "Die" based on priority.
- **Distributed Ready**: Works with the Klock CLI server to coordinate agents across different servers or containers.

## Installation

```bash
pip install klock
```

## Quick Start

```python
from klock import KlockClient

# 1. Connect to your Klock daemon
client = KlockClient("http://localhost:3100")

# 2. Register an agent with a priority (lower = senior)
client.register_agent("refactor-bot", 100)

# 3. Acquire a lease for a resource
result = client.acquire_lease(
    agent_id="refactor-bot",
    session_id="session-456",
    resource_type="FILE",
    resource_path="src/main.py",
    predicate="MUTATES",
    ttl=60000
)

if result["success"]:
    print(f"Lease acquired: {result['lease_id']}")
    # Do work...
    client.release_lease(result["lease_id"])
else:
    print(f"Conflict: {result['reason']}")
```

## License

MIT
