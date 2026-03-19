# JavaScript SDK

The JavaScript SDK now exposes two entrypoints.

## `KlockClient`

Use this for embedded coordination inside one Node process.

```javascript
const { KlockClient } = require('@klock-protocol/core');

const klock = new KlockClient();
klock.registerAgent('agent-a', 100);
const result = JSON.parse(
  klock.acquireLease('agent-a', 'session-a', 'FILE', '/src/auth.js', 'MUTATES', 5000),
);
```

## `KlockHttpClient`

Use this for the local-server OSS v1 workflow.

```javascript
const { KlockHttpClient } = require('@klock-protocol/core');

const klock = new KlockHttpClient({ baseUrl: 'http://localhost:3100' });
await klock.registerAgent('agent-a', 100);
const result = await klock.acquireLease(
  'agent-a',
  'session-a',
  'FILE',
  '/src/auth.js',
  'MUTATES',
  5000,
);
```

When the client targets `localhost`, it auto-starts the local server by default using:

1. `KLOCK_SERVER_COMMAND`
2. installed `klock` binary
3. source-tree `cargo run --release -p klock-cli -- serve`

When auto-start happens, the SDK logs:

- the base URL
- the launch command
- the PID of the spawned server

Disable auto-start with either:

- `KLOCK_DISABLE_AUTOSTART=1`
- `new KlockHttpClient({ autoStart: false })`

### Available methods

- `registerAgent(agentId, priority)`
- `acquireLease(agentId, sessionId, resourceType, resourcePath, predicate, ttl)`
- `releaseLease(leaseId)`
- `heartbeatLease(leaseId)`
- `listLeases()`

Useful runtime fields:

- `autoStart`
- `autoStartDisabledByEnv`
- `autoStartAttempted`
- `autoStartedPid`
