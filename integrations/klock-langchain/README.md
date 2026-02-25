# klock-langchain

[![PyPI version](https://badge.fury.io/py/klock-langchain.svg)](https://badge.fury.io/py/klock-langchain)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

Official LangChain integration for **Klock**, the coordination infrastructure that prevents Multi-Agent Race Conditions (MARC).

This package provides the `@klock_protected` decorator, allowing you to wrap any LangChain `BaseTool` with Klock's Wait-Die concurrency control. This ensures that when multiple autonomous agents try to modify the same resource simultaneously, they do not corrupt your data or cause silent data loss.

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

# Initialize your Klock kernel client
klock_client = KlockClient("http://localhost:8080")

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

## License

MIT License
