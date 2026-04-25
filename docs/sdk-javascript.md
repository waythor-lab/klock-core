# JavaScript SDK

The default JavaScript surface is `Klock`, a facade that hides the local coordinator unless you need advanced control.

## `Klock.local`

Use this for normal OSS v1 repo coordination. It uses the local coordinator under the hood, so separate agents and processes share the same lease view.

```javascript
const { Klock } = require('@klock-protocol/core');

const klock = Klock.local({ agentId: 'agent-a' });

await klock.withFile('/src/auth.js', { mode: 'mutate' }, async () => {
  // read or write the file safely
});
```

Supported file modes:

- `read`
- `mutate`
- `delete`
- `rename`
- `provide`
- `depend`

## `Klock.embedded`

Use this for tests and single-process demos. It does not coordinate with other processes.

```javascript
const { Klock } = require('@klock-protocol/core');

const klock = Klock.embedded({ agentId: 'test-agent' });
```

## Advanced Clients

`KlockClient` is the embedded low-level client. `KlockHttpClient` is the HTTP-backed low-level client.

```javascript
const { KlockClient, KlockHttpClient } = require('@klock-protocol/core');

const embedded = new KlockClient();
const remote = new KlockHttpClient({ baseUrl: 'http://localhost:3100' });
```

`KlockHttpClient` auto-starts the local server for localhost workflows. Disable auto-start with `KLOCK_DISABLE_AUTOSTART=1` or `new KlockHttpClient({ autoStart: false })`.
