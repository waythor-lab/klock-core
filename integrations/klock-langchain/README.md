# klock-langchain

`klock-langchain` is the canonical OSS v1 integration for Klock.

It coordinates LangChain file-mutating tools so multiple agents can work in the same repo without silently overwriting each other.

This is coordination for cooperative tools that call Klock before mutating shared files. It is not yet filesystem-level enforcement.

## Install

```bash
pip install klock klock-langchain langchain-core
```

## Local workflow

For localhost workflows, `KlockHttpClient` now auto-starts the local server when it can find a launch command.

When auto-start happens, the SDK logs the base URL, launch command, and PID.

Disable auto-start with:

- `KLOCK_DISABLE_AUTOSTART=1`
- `KlockHttpClient(..., auto_start=False)`

Then wrap your tool:

```python
from klock import KlockHttpClient
from klock_langchain import KlockConflictError, klock_protected
from langchain_core.tools import BaseTool

klock = KlockHttpClient(base_url="http://localhost:3100")
klock.register_agent("agent-a", 100)

class WriteFileTool(BaseTool):
    name = "write_file"
    description = "Mutates a repo file."

    @klock_protected(
        klock_client=klock,
        agent_id="agent-a",
        session_id="repo-session-a",
        resource_type="FILE",
        resource_path_extractor=lambda kwargs: kwargs["path"],
        predicate="MUTATES",
    )
    def _run(self, path: str, content: str) -> str:
        with open(path, "w", encoding="utf-8") as handle:
            handle.write(content)
        return f"updated {path}"
```

## Conflict behavior

`klock_protected(...)` uses Wait-Die semantics from the Klock server:

- `GRANT`: the tool enters the critical section and runs
- `WAIT`: the decorator sleeps for the server-provided backoff and retries
- `DIE`: the decorator raises `KlockConflictError` so the caller can retry later

Example recovery loop:

```python
try:
    tool.invoke({"path": "src/auth.js", "content": "..."})
except KlockConflictError as exc:
    if exc.reason == "DIE":
        # retry later
        ...
```

## Proof assets

The repo includes deterministic proof scripts built around the same local-server workflow:

- [examples/oss_v1/without_klock.py](/Users/nossairmouad/Projects/Klock/Klock-OpenSource/examples/oss_v1/without_klock.py)
- [examples/oss_v1/with_klock.py](/Users/nossairmouad/Projects/Klock/Klock-OpenSource/examples/oss_v1/with_klock.py)
- [examples/oss_v1/wait_die_trace.py](/Users/nossairmouad/Projects/Klock/Klock-OpenSource/examples/oss_v1/wait_die_trace.py)
- [examples/oss_v1/langchain_base_tool_demo.py](/Users/nossairmouad/Projects/Klock/Klock-OpenSource/examples/oss_v1/langchain_base_tool_demo.py)

Run them from [examples/oss_v1/README.md](/Users/nossairmouad/Projects/Klock/Klock-OpenSource/examples/oss_v1/README.md).
