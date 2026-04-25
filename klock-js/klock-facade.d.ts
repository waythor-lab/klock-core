export type KlockFileMode =
  | 'read'
  | 'mutate'
  | 'delete'
  | 'rename'
  | 'provide'
  | 'depend'

export interface KlockLocalOptions {
  agentId?: string
  sessionId?: string
  priority?: number
  baseUrl?: string
  apiKey?: string | null
  timeoutMs?: number
  autoStart?: boolean
  startupTimeoutMs?: number
  serverCommand?: readonly string[]
}

export interface KlockEmbeddedOptions {
  agentId?: string
  sessionId?: string
  priority?: number
}

export interface KlockFileOptions {
  mode?: KlockFileMode
  ttlMs?: number
  maxRetries?: number
}

export declare class Klock {
  static local(options?: KlockLocalOptions): Klock
  static embedded(options?: KlockEmbeddedOptions): Klock
  registerAgent(agentId: string, priority: number): Promise<void>
  acquireLease(
    agentId: string,
    sessionId: string,
    resourceType: string,
    resourcePath: string,
    predicate: string,
    ttl: number,
  ): Promise<AcquireLeaseResult>
  releaseLease(leaseId: string): Promise<boolean>
  withFile<T>(
    path: string,
    options: KlockFileOptions,
    callback: () => T | Promise<T>,
  ): Promise<T>
  withFile<T>(path: string, callback: () => T | Promise<T>): Promise<T>
}
