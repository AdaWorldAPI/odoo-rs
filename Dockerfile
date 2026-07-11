# syntax=docker/dockerfile:1
# ---------------------------------------------------------------------------
# odoo-rs — Railway / PaaS container.
#
# Serves the Odoo→OGAR transpiler frontend over HTTP: `od-server` binds
# 0.0.0.0:$PORT and shells to `od-codegen` (the proven CLI) for classid
# resolution + action-row lowering. Two binaries, one image.
#
# Build args pinned to the repo's toolchain (rust-toolchain.toml → 1.95).
# ---------------------------------------------------------------------------

FROM rust:1.95-bookworm AS builder
WORKDIR /build

# Copy the whole workspace (Cargo.lock + all crates) so both binaries build
# against one resolved graph.
COPY . .

# od-codegen requires the `cli` feature (its bin is `required-features = ["cli"]`).
# od-server has no features. Build both in release.
RUN cargo build --release --features cli --bin od-codegen \
 && cargo build --release --bin od-server

# ---------------------------------------------------------------------------
FROM debian:bookworm-slim AS runtime
WORKDIR /app

# od-codegen is pure-Rust (serde only) and od-server links libc + openssl-free
# axum/tokio; the slim base + ca-certificates is enough.
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates \
 && rm -rf /var/lib/apt/lists/*

COPY --from=builder /build/target/release/od-codegen /usr/local/bin/od-codegen
COPY --from=builder /build/target/release/od-server  /usr/local/bin/od-server
# Bundle the committed SPO corpora so a fresh deploy can transpile immediately.
COPY --from=builder /build/data /app/data

# Railway injects $PORT; od-server reads it and binds 0.0.0.0:$PORT.
# od-server finds od-codegen on PATH (installed above).
ENV PORT=8080
EXPOSE 8080

CMD ["od-server"]
