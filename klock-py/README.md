# klock

Python SDK for Klock OSS v1.

Use the high-level facade first:

```python
from klock import Klock

klock = Klock.local(agent_id="agent-a")

with klock.file("/src/auth.js", mode="mutate"):
    # read/write safely
    ...
```

`Klock.local(...)` coordinates through the local Klock server under the hood and auto-starts it on the first lock operation for localhost workflows. Use `Klock.embedded(...)` only when all coordinated work happens inside one process.

Advanced clients are still available:

- `KlockClient` for embedded low-level coordination
- `KlockHttpClient` for direct HTTP coordinator access
