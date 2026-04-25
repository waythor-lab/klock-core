# Changelog

All notable changes to Klock are documented here. Versions follow semver.

## 0.1.3 — Hardening pass

This release is a coordinated hardening pass across the kernel, server, SDKs,
and integrations. No public APIs are removed; one new failure mode and a few
new methods are added. Drop-in replacement for v0.1.2.

### Coordination correctness

- **Equal priorities now tie-break by `agent_id` lexicographically** instead
  of falling into a symmetric DIE loop. The Wait-Die ordering becomes the
  strict pair `(priority, agent_id)`. Older agent still wins.
- **Intent ledger removed; intents are derived from active leases.** The
  client no longer maintains a separate `active_intents` Vec — it was
  growing unbounded because the v0.1.2 cleanup matched lease IDs against
  intent IDs that were generated independently. The kernel now derives its
  conflict view from active leases at snapshot time. Single source of truth.
- **`acquire()` no longer reports phantom Success on persistence failure.**
  The SQLite store now checks the row count of every `INSERT` and surfaces
  failures as a typed `LeaseFailureReason::StorageUnavailable`.

### Storage failure model

- **New variant: `LeaseFailureReason::StorageUnavailable`** (HTTP 503,
  `STORAGE_UNAVAILABLE` over the wire). Distinct from coordination
  conflicts (`Wait`/`Die`/`Conflict`/etc., which stay HTTP 409).
- **SQLite store now fails closed via a poisoned flag.** A read or persist
  failure flips the store into a state where subsequent `acquire()` calls
  return `StorageUnavailable` immediately rather than scheduling against a
  stale or empty lease view (which could grant conflicting work).
- **No more `.expect()` panics in the SQLite store.** Read failures log
  via `tracing` and poison the store instead of crashing the server.
- **`/health` returns 503 when poisoned**, so a load balancer can pull the
  node.
- **`register_agent_priority` no longer diverges in-memory vs disk.** If
  the disk write fails, the in-memory map is left untouched (otherwise
  priorities would silently regress on server restart).
- New `KlockClient::storage_poisoned() -> bool` accessor exposes the flag.

### Server hardening (klock-cli)

- **Constant-time auth comparison** via the `subtle` crate. The previous
  `==` comparison was vulnerable to timing attacks.
- **Case-insensitive `Bearer ` prefix** in `Authorization` headers, per
  RFC 7235.
- **Payload size caps** on `agent_id` (256 B), `session_id` (256 B),
  `resource_path` (4 KiB), and `intents` count (256). Oversized requests
  return 400 without echoing the offending value.
- **HTTP integration test suite** added (`klock-cli/tests/integration_test.rs`)
  covering the happy path, auth, payload caps, and conflicting acquires.

### Kernel: ResourceRef path normalization

- File paths are now trimmed and have runs of `/` collapsed and trailing
  `/` stripped. Two callers using `/src/foo.ts ` and `/src//foo.ts` now
  hit the same lease entry. Case is preserved (file systems on Linux/macOS
  are case-sensitive).

### Python SDK (`klock`)

- **`KlockHttpClient` now retains its child server handle.** A spawned
  `klock serve` subprocess is terminated by `shutdown()`, the
  context-manager protocol, or `Drop`. v0.1.2 dropped the handle on the
  floor and orphaned the server when the parent Python process exited.
- New `shutdown()` method.
- New `__enter__`/`__exit__` so `with KlockHttpClient(...) as klock:` works.
- New `STORAGE_UNAVAILABLE` failure reason mapped through the FFI.
- Type stubs updated to declare the new methods.

### JavaScript SDK (`@klock-protocol/core`)

- **`KlockHttpClient` is now in `index.d.ts`.** v0.1.2 only declared the
  embedded `KlockClient`, so `import { KlockHttpClient }` failed in
  TypeScript. The HTTP client and its option, success/failure, predicate,
  and resource-type types are all declared.
- New `STORAGE_UNAVAILABLE` failure reason in the type union.
- Type-level smoke test (`__test__/types-smoke.ts`) added to guard the
  `.d.ts` against regression. Run via `npm run test:types`.
- `KlockHttpClient` JS implementation moved to `klock-http-client.js` and
  spliced onto NAPI's auto-generated `index.js` via a `postbuild` script,
  so `napi build` no longer wipes the JS-only client.

### LangChain integration (`klock-langchain`)

- **Async support: `@klock_protected` now wraps `_arun` (coroutine
  functions) too.** Detected via `inspect.iscoroutinefunction`. The async
  path uses `await asyncio.sleep` for WAIT backoff so the event loop
  stays responsive.
- **Extractor exceptions become typed `ValueError`.** A
  `resource_path_extractor` that raises `KeyError` no longer leaks the raw
  exception type through the decorator.
- New async test file (`tests/test_async.py`) with full coverage.
- Dependency pin bumped: `klock>=0.1.3`.

### Documentation

- `docs/architecture.md`: tightened the O(1) framing (matrix lookup is
  O(1); full conflict scan is O(n)) and the Wait-Die liveness phrasing
  (acknowledges the equal-priority tiebreak).

### Tests added

- Scheduler: equal-priority tiebreak both directions + priority-dominates-agent_id regression.
- Client: declare_intent non-accumulation, release-lease intent-view-clear,
  unknown-id no-panic, in-memory store never poisoned.
- SQLite: happy path, dropped-table read failure poisons, fail-closed
  acquire after poison, INSERT failure returns `StorageUnavailable`,
  register_agent_priority does not diverge on failure.
- Concurrency: 100 thread + tokio task contention, exactly one Success.
- HTTP integration: 9 tests across health, register/acquire/release,
  auth (missing / wrong / right / lowercase Bearer), payload caps,
  conflicting acquires.
- JS: type-defs smoke test compiles `index.d.ts` against a usage snippet.
- LangChain async: 6 new tests covering all paths.

### Out of scope (deferred to a later release)

- Migrating the `LeaseStore` trait to `Result<_, StorageError>`.
- Atomic SQLite acquire via transaction + uniqueness constraint (only
  matters for multi-process / shared-DB deployments).
- Unifying `declare_intent` with lease creation.
- Rate-limiting middleware.
- CORS hardening for remote deployment.
