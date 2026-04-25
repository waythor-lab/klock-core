# Python SDK

The default Python surface is `Klock`, a small facade for protecting local file operations.

## `Klock.local`

Use this for normal OSS v1 repo coordination. It uses the local coordinator under the hood, so separate agents and processes share the same lease view.

```python
from klock import Klock

klock = Klock.local(agent_id="agent-a")

with klock.file("/src/auth.js", mode="mutate"):
    # read or write the file safely
    ...
```

Supported file modes:

- `read`
- `mutate`
- `delete`
- `rename`
- `provide`
- `depend`

`Klock.local(...)` auto-starts the local server on the first lock/register/acquire operation for localhost workflows using:

1. `KLOCK_SERVER_COMMAND`
2. installed `klock` binary
3. source-tree `cargo run --release -p klock-cli -- serve`

Disable auto-start with `KLOCK_DISABLE_AUTOSTART=1` or by using the advanced `KlockHttpClient(..., auto_start=False)`.

## `Klock.embedded`

Use this for tests, notebooks, and single-process demos. It does not coordinate with other processes.

```python
from klock import Klock

klock = Klock.embedded(agent_id="test-agent")
```

## Advanced Clients

`KlockClient` is the embedded low-level client. `KlockHttpClient` is the HTTP-backed low-level client.

```python
from klock import KlockClient, KlockHttpClient

embedded = KlockClient()
remote = KlockHttpClient("http://localhost:3100")
```

For LangChain, pass the facade directly:

```python
from klock import Klock
from klock_langchain import klock_protected

klock = Klock.local(agent_id="agent-a")
```
