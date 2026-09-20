# syntax=docker/dockerfile:1.6
FROM rust:1.82-bookworm AS builder
WORKDIR /app
# Copy backend manifests first for layer cache.
COPY backend/Cargo.toml ./backend/Cargo.toml
COPY backend/rustfmt.toml ./backend/rustfmt.toml
COPY backend/crates/alice-core/Cargo.toml ./backend/crates/alice-core/Cargo.toml
COPY backend/crates/alice-daemon/Cargo.toml ./backend/crates/alice-daemon/Cargo.toml
# Create dummy sources to cache dependencies.
RUN mkdir -p backend/crates/alice-core/src backend/crates/alice-daemon/src && \
    echo "pub fn dummy(){}" > backend/crates/alice-core/src/lib.rs && \
    echo "pub mod config; pub mod dto; pub mod error;" > backend/crates/alice-core/src/lib.rs || true && \
    printf "fn main(){}" > backend/crates/alice-daemon/src/main.rs && \
    cargo fetch --manifest-path backend/Cargo.toml || true
# Copy real sources.
COPY backend ./backend
RUN cargo build --release --manifest-path backend/Cargo.toml

FROM debian:bookworm-slim AS runner
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates curl && \
    rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /app/backend/target/release/alice-daemon /usr/local/bin/alice-daemon
# Create data dir for sqlite file.
RUN mkdir -p /data
EXPOSE 8787
ENV DATABASE_URL=sqlite:///data/alice.db?mode=rwc
ENV ALICE_LISTEN_ADDR=0.0.0.0:8787
ENV RUST_LOG=info
HEALTHCHECK --interval=10s --timeout=3s --retries=5 CMD curl -f http://127.0.0.1:8787/api/health || exit 1
CMD ["alice-daemon"]
