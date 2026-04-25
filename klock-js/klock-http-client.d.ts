/*
 * Hand-maintained TypeScript declarations for the JS-only KlockHttpClient.
 *
 * This file is concatenated onto NAPI-RS's auto-generated `index.d.ts` by
 * the `postbuild` script. Do NOT edit `index.d.ts` directly — NAPI rewrites
 * it on every build.
 */

/** Constructor options for the HTTP-backed client. */
export interface KlockHttpClientOptions {
  /** Base URL of the Klock server. Defaults to "http://localhost:3100". */
  baseUrl?: string
  /** Bearer token for the `Authorization` header. Optional. */
  apiKey?: string | null
  /** Per-request timeout in milliseconds. Defaults to 5000. */
  timeoutMs?: number
  /** Auto-start a local server when the URL is localhost and the server is unreachable. Defaults to true. */
  autoStart?: boolean
  /** Total startup timeout for auto-start, in milliseconds. Defaults to 5000. */
  startupTimeoutMs?: number
  /** Command + args used to spawn the local server. Defaults to a sane built-in. */
  serverCommand?: readonly string[]
}

export type KlockResourceType =
  | 'FILE'
  | 'SYMBOL'
  | 'API_ENDPOINT'
  | 'DATABASE_TABLE'
  | 'CONFIG_KEY'
  | (string & {})

export type KlockPredicate =
  | 'PROVIDES'
  | 'CONSUMES'
  | 'MUTATES'
  | 'DELETES'
  | 'DEPENDS_ON'
  | 'RENAMES'
  | (string & {})

export type KlockFailureReason =
  | 'WAIT'
  | 'DIE'
  | 'CONFLICT'
  | 'RESOURCE_LOCKED'
  | 'SESSION_EXPIRED'
  | 'STORAGE_UNAVAILABLE'
  | (string & {})

export interface AcquireLeaseSuccess {
  success: true
  leaseId: string
  agentId: string
  resource: string
  predicate: KlockPredicate
  expiresAt: number
}

export interface AcquireLeaseFailure {
  success: false
  reason: KlockFailureReason
  waitTime: number
}

export type AcquireLeaseResult = AcquireLeaseSuccess | AcquireLeaseFailure

export interface ActiveLeaseInfo {
  id: string
  agentId: string
  resource: string
  predicate: KlockPredicate
  expiresAt: number
}

/**
 * HTTP-backed client for a local or remote Klock coordination server.
 * Uses the same wire protocol as the Python `KlockHttpClient`.
 */
export declare class KlockHttpClient {
  constructor(options?: KlockHttpClientOptions)
  /** Register an agent priority on the server. Lower = older = higher priority. */
  registerAgent(agentId: string, priority: number): Promise<void>
  /** Acquire a lease. Resolves to either a success or a typed failure (no throw on conflict). */
  acquireLease(
    agentId: string,
    sessionId: string,
    resourceType: KlockResourceType,
    resourcePath: string,
    predicate: KlockPredicate,
    ttl: number,
  ): Promise<AcquireLeaseResult>
  /** Release a held lease. Returns true if the server reported success. */
  releaseLease(leaseId: string): Promise<boolean>
  /** Refresh a held lease's TTL. */
  heartbeatLease(leaseId: string): Promise<boolean>
  /** List all currently active leases. */
  listLeases(): Promise<ActiveLeaseInfo[]>
}
