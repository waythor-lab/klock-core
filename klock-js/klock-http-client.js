/*
 * KlockHttpClient: HTTP-backed client for a local or remote Klock server.
 *
 * Hand-maintained JS implementation. Glued onto NAPI-RS's auto-generated
 * `index.js` by `scripts/postbuild.mjs` so that `import { KlockHttpClient }`
 * works alongside the embedded native `KlockClient`.
 */
'use strict'

const { existsSync } = require('fs')
const { join } = require('path')
const { spawn, spawnSync } = require('child_process')

class KlockHttpClient {
  constructor(options = {}) {
    this.autoStartDisabledByEnv = isAutoStartDisabledByEnv()
    this.baseUrl = (options.baseUrl || 'http://localhost:3100').replace(/\/+$/, '')
    this.apiKey = options.apiKey || null
    this.timeoutMs = options.timeoutMs || 5000
    this.autoStart = options.autoStart !== false && !this.autoStartDisabledByEnv
    this.startupTimeoutMs = options.startupTimeoutMs || 5000
    this.serverCommand = options.serverCommand || defaultServerCommand()
    this.autoStartAttempted = false
    this.autoStartedPid = null
  }

  async registerAgent(agentId, priority) {
    const response = await this.#request('POST', '/agents', {
      agent_id: agentId,
      priority,
    })

    if (!response.success) {
      throw new Error(response.error || response.reason || 'Failed to register Klock agent')
    }
  }

  async acquireLease(agentId, sessionId, resourceType, resourcePath, predicate, ttl) {
    const response = await this.#request('POST', '/leases', {
      agent_id: agentId,
      session_id: sessionId,
      resource_type: resourceType,
      resource_path: resourcePath,
      predicate,
      ttl,
    })

    if (response.success) {
      return {
        success: true,
        leaseId: response.data.lease_id,
        agentId: response.data.agent_id,
        resource: response.data.resource,
        predicate: response.data.predicate,
        expiresAt: response.data.expires_at,
      }
    }

    return {
      success: false,
      reason: response.reason || 'CONFLICT',
      waitTime: response.wait_time ?? 1000,
    }
  }

  async releaseLease(leaseId) {
    const response = await this.#request('DELETE', `/leases/${leaseId}`)
    return Boolean(response.success)
  }

  async heartbeatLease(leaseId) {
    const response = await this.#request('POST', `/leases/${leaseId}/heartbeat`)
    return Boolean(response.success)
  }

  async listLeases() {
    const response = await this.#request('GET', '/leases')
    if (!response.success) {
      throw new Error(response.error || response.reason || 'Failed to list Klock leases')
    }

    return (response.data || []).map((lease) => ({
      id: lease.id,
      agentId: lease.agent_id,
      resource: lease.resource,
      predicate: lease.predicate,
      expiresAt: lease.expires_at,
    }))
  }

  async #request(method, path, payload) {
    if (path !== '/health') {
      await this.#ensureServer()
    }

    const controller = new AbortController()
    const timeout = setTimeout(() => controller.abort(), this.timeoutMs)
    const headers = {}

    if (this.apiKey) {
      headers.authorization = `Bearer ${this.apiKey}`
    }
    if (payload !== undefined) {
      headers['content-type'] = 'application/json'
    }

    try {
      const response = await fetch(`${this.baseUrl}${path}`, {
        method,
        headers,
        body: payload === undefined ? undefined : JSON.stringify(payload),
        signal: controller.signal,
      })

      const raw = await response.text()
      return raw ? JSON.parse(raw) : {}
    } catch (error) {
      throw new Error(`Failed to reach Klock server at ${this.baseUrl}: ${error.message}`)
    } finally {
      clearTimeout(timeout)
    }
  }

  async #ensureServer() {
    if (await this.#healthCheck()) {
      return
    }

    if (!isLocalBaseUrl(this.baseUrl)) {
      return
    }

    if (!this.autoStart) {
      const disabledReason = this.autoStartDisabledByEnv ? ' by KLOCK_DISABLE_AUTOSTART' : ''
      throw new Error(
        `No Klock server is reachable at ${this.baseUrl}. Auto-start is disabled${disabledReason}. ` +
          `Start it manually with: ${formatServerCommand(this.serverCommand)}`
      )
    }

    if (this.autoStartAttempted) {
      const pidNote = this.autoStartedPid == null ? '' : ` Last attempted PID: ${this.autoStartedPid}.`
      throw new Error(
        `Klock server is still unavailable at ${this.baseUrl} after an earlier auto-start attempt.` +
          `${pidNote} Start it manually with: ${formatServerCommand(this.serverCommand)}`
      )
    }

    const [program, ...args] = this.serverCommand
    if (!program) {
      throw new Error('Klock auto-start is enabled but no server command is configured')
    }

    console.error(
      `Starting local Klock server for ${this.baseUrl} using: ${formatServerCommand(this.serverCommand)}`
    )

    const child = spawn(program, args, {
      stdio: 'ignore',
    })
    const spawnError = await new Promise((resolve) => {
      child.once('spawn', () => resolve(null))
      child.once('error', (error) => resolve(error))
    })
    if (spawnError) {
      throw new Error(
        `Failed to auto-start Klock server with command ${formatServerCommand(this.serverCommand)}: ${spawnError.message}`
      )
    }

    this.autoStartAttempted = true
    this.autoStartedPid = child.pid ?? null
    console.error(
      `Started local Klock server for ${this.baseUrl} with PID ${this.autoStartedPid ?? 'unknown'}`
    )
    child.unref()

    const deadline = Date.now() + this.startupTimeoutMs
    while (Date.now() < deadline) {
      if (child.exitCode !== null || child.signalCode !== null) {
        const exitDetails =
          child.exitCode !== null
            ? `exit code ${child.exitCode}`
            : `signal ${child.signalCode}`
        throw new Error(
          `Klock server process exited before becoming healthy at ${this.baseUrl} with ${exitDetails}. ` +
            `Start it manually with: ${formatServerCommand(this.serverCommand)}`
        )
      }

      if (await this.#healthCheck()) {
        console.error(
          `Klock server is healthy at ${this.baseUrl} (PID ${this.autoStartedPid ?? 'unknown'})`
        )
        return
      }
      await new Promise((resolve) => setTimeout(resolve, 150))
    }

    throw new Error(
      `Klock server did not become healthy at ${this.baseUrl} within ${this.startupTimeoutMs}ms ` +
        `after auto-starting PID ${this.autoStartedPid ?? 'unknown'}. ` +
        `Start it manually with: ${formatServerCommand(this.serverCommand)}`
    )
  }

  async #healthCheck() {
    const controller = new AbortController()
    const timeout = setTimeout(() => controller.abort(), this.timeoutMs)
    const headers = {}
    if (this.apiKey) {
      headers.authorization = `Bearer ${this.apiKey}`
    }

    try {
      const response = await fetch(`${this.baseUrl}/health`, {
        method: 'GET',
        headers,
        signal: controller.signal,
      })
      return response.ok
    } catch {
      return false
    } finally {
      clearTimeout(timeout)
    }
  }
}

function defaultServerCommand() {
  if (process.env.KLOCK_SERVER_COMMAND) {
    return process.env.KLOCK_SERVER_COMMAND.trim().split(/\s+/)
  }

  if (commandExists('klock')) {
    return ['klock', 'serve']
  }

  const workspaceRoot = findWorkspaceRoot()
  if (workspaceRoot) {
    return [
      'cargo',
      'run',
      '--release',
      '--manifest-path',
      join(workspaceRoot, 'Cargo.toml'),
      '-p',
      'klock-cli',
      '--',
      'serve',
    ]
  }

  return ['klock', 'serve']
}

function isAutoStartDisabledByEnv() {
  const value = process.env.KLOCK_DISABLE_AUTOSTART
  if (!value) {
    return false
  }

  return ['1', 'true', 'yes', 'on'].includes(value.trim().toLowerCase())
}

function formatServerCommand(command) {
  if (!command || command.length === 0) {
    return '<no command configured>'
  }

  return command.join(' ')
}

function commandExists(command) {
  const result = spawnSync(command, ['--help'], { stdio: 'ignore' })
  return !result.error
}

function findWorkspaceRoot() {
  let cwd = process.cwd()
  while (true) {
    if (existsSync(join(cwd, 'Cargo.toml')) && existsSync(join(cwd, 'klock-cli'))) {
      return cwd
    }

    const parent = join(cwd, '..')
    if (parent === cwd) {
      return null
    }
    cwd = parent
  }
}

function isLocalBaseUrl(baseUrl) {
  return baseUrl.includes('localhost') || baseUrl.includes('127.0.0.1')
}

module.exports = { KlockHttpClient }
