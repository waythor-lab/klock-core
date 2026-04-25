// Type-only smoke test for klock-js TypeScript declarations.
// Compiled with `tsc --noEmit`; never executed.
//
// This guards the contract that BOTH KlockClient and KlockHttpClient are
// exported from index.d.ts (the v0.1.2 d.ts only declared KlockClient,
// breaking TS users of the HTTP-backed client).

import type {
  KlockClient,
  KlockHttpClient,
  KlockHttpClientOptions,
  AcquireLeaseResult,
  AcquireLeaseSuccess,
  AcquireLeaseFailure,
  ActiveLeaseInfo,
  KlockResourceType,
  KlockPredicate,
  KlockFailureReason,
} from "../index.js";

// Unused-symbol references are kept compile-time only.
declare const embedded: KlockClient;
declare const http: KlockHttpClient;
declare const opts: KlockHttpClientOptions;
declare const result: AcquireLeaseResult;
declare const success: AcquireLeaseSuccess;
declare const failure: AcquireLeaseFailure;
declare const info: ActiveLeaseInfo;
declare const rt: KlockResourceType;
declare const pred: KlockPredicate;
declare const reason: KlockFailureReason;

// Spot-check method signatures for both clients.
async function _smoke(): Promise<void> {
  // Embedded
  embedded.registerAgent("a", 1);
  const lease = embedded.acquireLease("a", "s", "FILE", "/x", "MUTATES", 1000);
  const _l: string = lease;
  const released: boolean = embedded.releaseLease("id");
  const count: number = embedded.activeLeaseCount();
  const evicted: number = embedded.evictExpired();
  void released; void count; void evicted;

  // HTTP
  await http.registerAgent("a", 1);
  const r = await http.acquireLease("a", "s", "FILE", "/x", "MUTATES", 1000);
  if (r.success) {
    const _id: string = r.leaseId;
    void _id;
  } else {
    const _reason: KlockFailureReason = r.reason;
    void _reason;
  }
  const ok: boolean = await http.releaseLease("id");
  const beat: boolean = await http.heartbeatLease("id");
  const list: ActiveLeaseInfo[] = await http.listLeases();
  void ok; void beat; void list;

  // Reference the declared values so unused-import lints don't fire.
  void opts; void result; void success; void failure;
  void info; void rt; void pred; void reason;
}

void _smoke;
