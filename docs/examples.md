# Examples

Klock OSS v1 is centered on one concrete workflow: coordinate a shared repo file while multiple agents are editing it.

## Proof set

### 1. Without Klock

```bash
cd Klock-OpenSource/examples/oss_v1
python3 without_klock.py
```

This shows the failure mode the product is solving:

- two agents read the same file
- both write back successfully
- one update disappears without an error

### 2. With Klock

```bash
cd Klock-OpenSource/examples/oss_v1
python3 with_klock.py
```

This uses:

- the local `klock-cli` server
- the Python `Klock.local(...)` facade
- the `klock-langchain` decorator surface

The final workspace file preserves both edits because both agents cooperate with the Klock lease flow.

### 3. WAIT-DIE trace

```bash
cd Klock-OpenSource/examples/oss_v1
python3 wait_die_trace.py
```

This is the smallest direct protocol walkthrough that shows:

- `GRANT`
- `WAIT`
- `DIE`
- `GRANT` after release

## SDK snippets

### Python file guard

```python
from klock import Klock

klock = Klock.local(agent_id="agent-a")

with klock.file("/repo/src/auth.js", mode="mutate"):
    ...
```

### JavaScript file guard

```javascript
const { Klock } = require('@klock-protocol/core');

const klock = Klock.local({ agentId: 'agent-a' });

await klock.withFile('/repo/src/auth.js', { mode: 'mutate' }, async () => {
  // read/write safely
});
```
