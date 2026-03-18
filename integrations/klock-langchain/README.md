# klock-langchain

[![PyPI version](https://badge.fury.io/py/klock-langchain.svg)](https://badge.fury.io/py/klock-langchain)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

LangChain adapter for **Klock**, the coordination infrastructure that prevents Multi-Agent Race Conditions (MARC).

This package provides the `@klock_protected` decorator, allowing you to wrap any LangChain `BaseTool` with Klock's Wait-Die concurrency control. This ensures that when multiple autonomous agents try to modify the same resource simultaneously, they do not corrupt your data or cause silent data loss.

## Tiny Repro First

If you want to see the failure mode before touching LangChain, start with the smallest repro in the repo:

- [tiny_repro/race_condition.py](../../examples/tiny_repro/race_condition.py) — 2 workers, 1 file, deterministic silent overwrite
- [tiny_repro/klock_fixed.py](../../examples/tiny_repro/klock_fixed.py) — same workflow protected by Klock

From `Klock-OpenSource/examples/tiny_repro/`:

```bash
python3 race_condition.py
```

Then start the local Klock server from `Klock-OpenSource/`:

```bash
cargo run --release -p klock-cli -- serve
```

And in `examples/tiny_repro/` run:

```bash
python3 klock_fixed.py
```

The expected outcome is intentionally simple:

- without coordination: final state has **1** entry instead of **2**
- with Klock: final state has **2** entries

## Installation

```bash
pip install klock-langchain klock langchain-core
```

## Quick Start

Wrap your LangChain tools to enforce intent-based concurrency control before they execute:

```python
from langchain_core.tools import BaseTool
from klock import KlockClient
from klock_langchain import klock_protected

# Initialize a local Klock client and register this worker's priority
klock_client = KlockClient()
klock_client.register_agent("refactor-agent-1", 100)

# Define a tool and protect it with Klock
class WriteFileTool(BaseTool):
    name = "write_file"
    description = "Writes content to a file on disk"
    
    # Protect the _run method with Wait-Die concurrency control
    @klock_protected(
        klock_client=klock_client,
        agent_id="refactor-agent-1",
        session_id="session-123",
        resource_type="FILE",
        resource_path_extractor=lambda kwargs: kwargs.get("filepath"),
        predicate="MUTATES"
    )
    def _run(self, filepath: str, content: str) -> str:
        with open(filepath, 'w') as f:
            f.write(content)
        return f"Successfully wrote to {filepath}"
```

## How It Works

Klock uses **Wait-Die priority scheduling**, a classic database concurrency control algorithm, mapped specifically to LLM agents:
- **Older agents** wait for younger agents to finish (Wait).
- **Younger agents** abort immediately to prevent deadlocks (Die).

If a "Die" abort occurs, `klock_protected` raises a `RuntimeError`. LangChain's built-in error handling catches this and returns it to the LLM agent, allowing the agent to gracefully pause and retry the operation later.

This package is the current shipping integration surface. LangGraph and CrewAI can already use it at the tool layer even before dedicated adapters exist.

## Bigger Demos

After the tiny repro, the next assets are:

- `examples/openrouter_langchain/demo.py` for a simple LangChain + Klock flow
- `integrations/klock-langchain/real_agents_demo.py` for the larger crash-test style demonstration
- `examples/openrouter_langchain/demo_scale.py` for the multi-agent benchmark path

## License

MIT License
