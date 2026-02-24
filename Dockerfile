# ── Stage 1: Builder ──────────────────────────────────────────────────────────
FROM rust:1.83-slim AS builder

WORKDIR /build

# Copy workspace manifests first for caching
COPY Cargo.toml Cargo.lock ./
COPY klock-core/Cargo.toml klock-core/Cargo.toml
COPY klock-cli/Cargo.toml klock-cli/Cargo.toml

# Create dummy source files to cache dependencies
RUN mkdir -p klock-core/src klock-cli/src && \
    echo "pub fn dummy() {}" > klock-core/src/lib.rs && \
    echo "fn main() {}" > klock-cli/src/main.rs && \
    cargo build --release -p klock-cli 2>/dev/null || true

# Copy real source code
COPY klock-core/ klock-core/
COPY klock-cli/ klock-cli/

# Build the actual binary
RUN cargo build --release -p klock-cli

# ── Stage 2: Runtime ─────────────────────────────────────────────────────────
FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && \
    rm -rf /var/lib/apt/lists/*

COPY --from=builder /build/target/release/klock /usr/local/bin/klock

EXPOSE 3100

ENTRYPOINT ["klock"]
CMD ["serve", "--port", "3100"]
