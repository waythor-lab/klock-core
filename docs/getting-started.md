# Getting Started

Klock OSS v1 is a **local multi-agent repo coordination** workflow for cooperative agents.

This guide takes you from zero to a working proof:

1. reproduce the overwrite without Klock
2. let Klock auto-start the local server for localhost workflows
3. run the coordinated version
4. inspect `GRANT`, `WAIT`, and `DIE` behavior

## Prerequisites

- Python 3.8+
- Rust 1.75+ for the local `klock-cli` server

## 1. Install the packages

```bash
pip install klock klock-langchain
```

If you want the repo to run everything for you:

```bash
cd Klock-OpenSource
./scripts/run_oss_v1_demo.sh
```

That script creates a temp virtualenv, installs the local packages, auto-starts the local server when needed, and runs the proof set.

## 2. Reproduce the failure first

```bash
cd Klock-OpenSource/examples/oss_v1
python3 without_klock.py
```

Expected outcome:

- both agents print success
- only one feature block survives in `workspace/src/auth.js`
- the script ends with `SILENT OVERWRITE DETECTED`

## 3. Let localhost auto-start work by default

For local workflows, `KlockHttpClient("http://localhost:3100")` now tries to start `klock serve` automatically.

It uses this order:

1. `KLOCK_SERVER_COMMAND` if set
2. installed `klock` binary on your `PATH`
3. `cargo run --release -p klock-cli -- serve` when you are running from the source tree

When auto-start happens, the SDK logs the base URL and PID so the process is visible.

Disable auto-start with either:

- `KLOCK_DISABLE_AUTOSTART=1`
- `KlockHttpClient(..., auto_start=False)`

You can still start the server manually if you want, but it is no longer the default recommendation.

## 4. Run the coordinated version

```bash
cd Klock-OpenSource/examples/oss_v1
python3 with_klock.py
```

Expected outcome:

- both feature blocks survive
- the terminal may show the younger agent getting `DIE` and retrying
- the script ends with `WAIT-DIE COORDINATION CONFIRMED`

Important:

- this is coordination for agents that call the SDK or a wrapped tool
- it is not yet filesystem-level enforcement

## 5. Inspect the protocol directly

```bash
python3 wait_die_trace.py
```

Expected outcome:

- first request: `GRANT`
- older agent colliding with a younger holder: `WAIT`
- newest agent colliding with the older holder: `DIE`
- retry after release: `GRANT`

## 6. Validate the real LangChain tool surface

```bash
python3 langchain_base_tool_demo.py
```

Expected outcome:

- the demo uses a real `langchain_core.tools.BaseTool`
- both feature blocks survive
- the script ends with `LANGCHAIN TOOL PROTECTION CONFIRMED`

## 7. Adopt the LangChain path

```python
from klock import KlockHttpClient
from klock_langchain import klock_protected
from langchain_core.tools import BaseTool

klock = KlockHttpClient(base_url="http://localhost:3100")
klock.register_agent("refactor-bot", 100)

class WriteFileTool(BaseTool):
    name = "write_file"
    description = "Mutates a file inside a shared repo."

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

## Next steps

- [Examples](./examples.md)
- [Python SDK](./sdk-python.md)
- [Benchmarks and Proof](./benchmarks.md)
- [LangChain integration](../integrations/klock-langchain/README.md)
