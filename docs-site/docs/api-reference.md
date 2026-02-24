---
sidebar_position: 3
---

# REST API Reference

The `klock serve` command starts an Axum HTTP server that exposes a REST API for multi-agent coordination.

```bash
klock serve --port 3100 --host 0.0.0.0
```

---

## Endpoints

### `GET /health`

Health check. Returns server status and active lease count.

**Response:**
```json
{
  "success": true,
  "data": {
    "status": "ok",
    "active_leases": 3
  }
}
```

---

### `POST /agents`

Register an agent with a priority. Lower priority values = older = higher precedence in Wait-Die scheduling.

**Request:**
```json
{
  "agent_id": "refactor-bot",
  "priority": 100
}
```

**Response:**
```json
{
  "success": true,
  "data": {
    "agent_id": "refactor-bot",
    "priority": 100
  }
}
```

---

### `POST /leases`

Acquire a lease on a resource.

**Request:**
```json
{
  "agent_id": "refactor-bot",
  "session_id": "session-1",
  "resource_type": "FILE",
  "resource_path": "/src/auth.ts",
  "predicate": "MUTATES",
  "ttl": 60000
}
```

**Successful Response:**
```json
{
  "success": true,
  "data": {
    "lease_id": "abc123",
    "agent_id": "refactor-bot",
    "resource": "FILE:/src/auth.ts",
    "predicate": "Mutates",
    "expires_at": 1708700060000
  }
}
```

**Conflict Response (Wait-Die: Die):**
```json
{
  "success": false,
  "error": "Lease denied: DIE — Conflict: Senior (100) vs Junior (200). Junior must DIE."
}
```

#### Parameters

| Field | Type | Description |
|-------|------|-------------|
| `agent_id` | string | ID of the requesting agent |
| `session_id` | string | Session identifier (for reentrant lock logic) |
| `resource_type` | string | One of: `FILE`, `SYMBOL`, `API_ENDPOINT`, `DATABASE_TABLE`, `CONFIG_KEY` |
| `resource_path` | string | Path to the resource (e.g., `/src/auth.ts`) |
| `predicate` | string | One of: `PROVIDES`, `CONSUMES`, `MUTATES`, `DELETES`, `DEPENDS_ON`, `RENAMES` |
| `ttl` | integer | Time-to-live in milliseconds |

---

### `DELETE /leases/:id`

Release a lease by its ID.

**Response (success):**
```json
{
  "success": true,
  "data": {
    "released": true
  }
}
```

**Response (not found):**
```json
{
  "success": false,
  "error": "Lease not found"
}
```

---

### `GET /leases`

List all currently active leases.

**Response:**
```json
{
  "success": true,
  "data": [
    {
      "id": "abc123",
      "agent_id": "refactor-bot",
      "session_id": "session-1",
      "resource": { "resource_type": "File", "path": "/src/auth.ts" },
      "predicate": "Mutates",
      "state": "Active",
      "expires_at": 1708700060000
    }
  ]
}
```

---

### `POST /intents`

Declare an intent manifest and run it through the kernel.

**Request:**
```json
{
  "session_id": "session-1",
  "agent_id": "refactor-bot",
  "intents": [
    {
      "resource_type": "FILE",
      "resource_path": "/src/auth.ts",
      "predicate": "MUTATES"
    }
  ]
}
```

**Response:**
```json
{
  "success": true,
  "data": {
    "agent_id": "refactor-bot",
    "session_id": "session-1",
    "status": "Granted",
    "conflicts": []
  }
}
```

---

### `POST /evict`

Evict all expired leases.

**Response:**
```json
{
  "success": true,
  "data": {
    "evicted": 5
  }
}
```

---

## Response Format

All endpoints return this consistent envelope:

```json
{
  "success": true | false,
  "data": { ... },       // present on success
  "error": "..."          // present on failure
}
```

## CORS

The server enables permissive CORS (all origins, methods, headers) for local development.
