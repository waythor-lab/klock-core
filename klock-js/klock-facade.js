'use strict'

const { KlockHttpClient } = require('./klock-http-client')

const DEFAULT_BASE_URL = 'http://localhost:3100'
const DEFAULT_TTL_MS = 60_000
const DEFAULT_MAX_RETRIES = 10

function createKlockFacade(KlockClient) {
  class Klock {
    constructor({ client, agentId, sessionId, priority, embedded }) {
      this.client = client
      this.agentId = agentId || defaultAgentId()
      this.sessionId = sessionId || defaultSessionId()
      this.priority = priority ?? Date.now()
      this.embedded = embedded
      this.registered = false

      if (embedded) {
        this.client.registerAgent(this.agentId, this.priority)
        this.registered = true
      }
    }

    static local(options = {}) {
      return new Klock({
        client: new KlockHttpClient({
          baseUrl: options.baseUrl || DEFAULT_BASE_URL,
          apiKey: options.apiKey || null,
          timeoutMs: options.timeoutMs || 5000,
          autoStart: options.autoStart !== false,
          startupTimeoutMs: options.startupTimeoutMs || 5000,
          serverCommand: options.serverCommand,
        }),
        agentId: options.agentId,
        sessionId: options.sessionId,
        priority: options.priority,
        embedded: false,
      })
    }

    static embedded(options = {}) {
      return new Klock({
        client: new KlockClient(),
        agentId: options.agentId,
        sessionId: options.sessionId,
        priority: options.priority,
        embedded: true,
      })
    }

    async acquireLease(agentId, sessionId, resourceType, resourcePath, predicate, ttl) {
      await this.#ensureRegistered()
      if (this.embedded) {
        return normalizeEmbeddedResult(
          this.client.acquireLease(agentId, sessionId, resourceType, resourcePath, predicate, ttl)
        )
      }
      return this.client.acquireLease(agentId, sessionId, resourceType, resourcePath, predicate, ttl)
    }

    async registerAgent(agentId, priority) {
      if (this.embedded) {
        this.client.registerAgent(agentId, priority)
      } else {
        await this.client.registerAgent(agentId, priority)
      }
      if (agentId === this.agentId) {
        this.registered = true
      }
    }

    async releaseLease(leaseId) {
      if (this.embedded) {
        return this.client.releaseLease(leaseId)
      }
      return this.client.releaseLease(leaseId)
    }

    async withFile(path, options, callback) {
      let opts = options
      let fn = callback
      if (typeof options === 'function') {
        opts = {}
        fn = options
      }
      if (typeof fn !== 'function') {
        throw new TypeError('Klock.withFile requires a callback')
      }

      const mode = opts?.mode || 'mutate'
      const ttl = opts?.ttlMs || DEFAULT_TTL_MS
      const maxRetries = opts?.maxRetries ?? DEFAULT_MAX_RETRIES
      const predicate = modeToPredicate(mode)
      const leaseId = await this.#acquireWithRetries(path, predicate, ttl, maxRetries)

      try {
        return await fn()
      } finally {
        await this.releaseLease(leaseId)
      }
    }

    async #ensureRegistered() {
      if (this.registered) {
        return
      }
      await this.client.registerAgent(this.agentId, this.priority)
      this.registered = true
    }

    async #acquireWithRetries(path, predicate, ttl, maxRetries) {
      let retries = 0
      let totalWaitMs = 0

      while (true) {
        const result = await this.acquireLease(
          this.agentId,
          this.sessionId,
          'FILE',
          path,
          predicate,
          ttl
        )

        if (result.success) {
          return result.leaseId
        }

        if (result.reason === 'WAIT' && retries < maxRetries) {
          const waitMs = result.waitTime ?? 1000
          await sleep(waitMs)
          totalWaitMs += waitMs
          retries += 1
          continue
        }

        if (result.reason === 'WAIT') {
          throw new Error(
            `Klock exceeded max WAIT retries for ${this.agentId} on ${path} after ${totalWaitMs}ms`
          )
        }
        throw new Error(`Klock denied ${this.agentId} on ${path}: ${result.reason || 'CONFLICT'}`)
      }
    }
  }

  return Klock
}

function normalizeEmbeddedResult(raw) {
  const result = typeof raw === 'string' ? JSON.parse(raw) : raw
  if (result.success) {
    return {
      success: true,
      leaseId: result.leaseId,
      agentId: result.agentId,
      resource: result.resource,
      predicate: result.predicate,
      expiresAt: result.expiresAt,
    }
  }
  return {
    success: false,
    reason: result.reason || 'CONFLICT',
    waitTime: result.waitTime ?? 1000,
  }
}

function modeToPredicate(mode) {
  switch (String(mode).toLowerCase()) {
    case 'read':
      return 'CONSUMES'
    case 'mutate':
      return 'MUTATES'
    case 'delete':
      return 'DELETES'
    case 'rename':
      return 'RENAMES'
    case 'provide':
      return 'PROVIDES'
    case 'depend':
      return 'DEPENDS_ON'
    default:
      throw new Error(
        `Unsupported Klock file mode '${mode}'. Expected one of: read, mutate, delete, rename, provide, depend`
      )
  }
}

function defaultAgentId() {
  return `agent-${process.pid}`
}

function defaultSessionId() {
  return `session-${process.pid}-${Date.now()}`
}

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms))
}

module.exports = { createKlockFacade }
