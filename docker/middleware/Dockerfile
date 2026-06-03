# syntax=docker/dockerfile:1.7

# Build context is the repository root; invoke as:
#   docker build -t warpdrive-stellar-middleware:dev -f docker/middleware/Dockerfile .
#
# The *runtime* image ships a single native Rust binary (`warpdrive-deployer`)
# plus the compiled contract wasm — no shell scripts, no `stellar` CLI, no
# jq/curl/dbus. The *builder* still needs the `stellar` CLI because
# `task optimize` runs `stellar contract build --optimize` to produce the wasm;
# it is never copied into the runtime stage.
FROM rust:1.94-bookworm AS base

ARG STELLAR_CLI_VERSION=26.0.0
# Set automatically by BuildKit; resolved per build platform (amd64 / arm64).
ARG TARGETARCH

# stellar-cli binary is dynamically linked to libdbus (pulled in via hidapi).
RUN apt-get update \
    && apt-get install -y --no-install-recommends libdbus-1-3 \
    && rm -rf /var/lib/apt/lists/*

RUN rustup target add wasm32v1-none

RUN curl -sSL https://taskfile.dev/install.sh \
    | sh -s -- -d -b /usr/local/bin

# Install stellar-cli from the prebuilt release tarball so we don't have to
# compile its hidapi/dbus dependencies from source. Used by `task optimize` in
# the builder stage only.
RUN case "$TARGETARCH" in \
        amd64) STELLAR_TRIPLE="x86_64-unknown-linux-gnu" ;; \
        arm64) STELLAR_TRIPLE="aarch64-unknown-linux-gnu" ;; \
        *) echo "unsupported TARGETARCH: $TARGETARCH" >&2; exit 1 ;; \
    esac \
    && curl -fsSL -o /tmp/stellar-cli.tar.gz \
        "https://github.com/stellar/stellar-cli/releases/download/v${STELLAR_CLI_VERSION}/stellar-cli-${STELLAR_CLI_VERSION}-${STELLAR_TRIPLE}.tar.gz" \
    && tar -xzf /tmp/stellar-cli.tar.gz -C /usr/local/bin stellar \
    && rm /tmp/stellar-cli.tar.gz \
    && stellar --version

# Builder: BuildKit cache mounts keep cargo's registry + target dirs warm across
# local rebuilds. Artifacts are copied out of the (ephemeral) cache mounts in the
# same RUN so the runtime stage can see them. The deployer is excluded from the
# root workspace (its clap/tokio deps must not touch the wasm builds), so it has
# its own target dir and is built separately.
FROM base AS builder
WORKDIR /src
COPY . .
RUN --mount=type=cache,target=/usr/local/cargo/registry,id=warpdrive-cargo-registry \
    --mount=type=cache,target=/src/target,id=warpdrive-cargo-target \
    --mount=type=cache,target=/src/packages/deployer/target,id=warpdrive-deployer-target \
    task optimize \
    && cargo build --release --manifest-path packages/deployer/Cargo.toml \
    && mkdir -p /out/wasm \
    && cp target/wasm32v1-none/release/warpdrive_*.wasm /out/wasm/ \
    && cp packages/deployer/target/release/warpdrive-deployer /out/warpdrive-deployer

# Runtime stage: slim image carrying only the binary, the wasm, and the TLS roots
# needed to reach the RPC + friendbot over HTTPS.
FROM debian:bookworm-slim AS runtime

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /out/warpdrive-deployer /usr/local/bin/warpdrive-deployer

RUN mkdir -p /warpdrive/wasm
COPY --from=builder /out/wasm/ /warpdrive/wasm/

WORKDIR /warpdrive
# Long-lived container; callers issue commands via
#   docker exec <id> warpdrive-deployer <subcommand> ...
CMD ["sleep", "infinity"]
