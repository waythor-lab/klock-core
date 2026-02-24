---
sidebar_position: 1
---

# Docker Deployment

Klock provides a multi-stage Dockerfile for deploying the coordination server as a container.

## Quick Start

```bash
# Build the image
docker build -t klock-server .

# Run on port 3100
docker run -p 3100:3100 klock-server

# Custom port
docker run -p 8080:8080 klock-server serve --port 8080
```

## Dockerfile Architecture

```dockerfile
# Stage 1: Build (rust:1.83-slim)
# - Compiles klock-cli in release mode
# - Produces a single static binary

# Stage 2: Runtime (debian:bookworm-slim)
# - Only copies the binary (no Rust toolchain)
# - Minimal image size
```

| Stage | Base Image | Size |
|-------|-----------|------|
| Builder | `rust:1.83-slim` | ~1.4 GB |
| Runtime | `debian:bookworm-slim` | ~80 MB |
| **Final image** | — | **< 100 MB** |

## Docker Compose

For local development with logging:

```yaml
version: '3.8'
services:
  klock:
    build: .
    ports:
      - "3100:3100"
    environment:
      - RUST_LOG=info
    restart: unless-stopped
```

## Health Check

```bash
# Verify the container is running
curl http://localhost:3100/health

# Expected response:
# {"success":true,"data":{"status":"ok","active_leases":0}}
```

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `RUST_LOG` | `info` | Log level (`trace`, `debug`, `info`, `warn`, `error`) |

## CLI Arguments

The container's entrypoint runs `klock serve` by default. You can pass arguments:

```bash
# Custom port and host
docker run -p 8080:8080 klock-server serve --port 8080 --host 0.0.0.0

# Show version
docker run klock-server version

# One-shot conflict check (pipe JSON via stdin)
echo '{"intents": [...]}' | docker run -i klock-server check
```
