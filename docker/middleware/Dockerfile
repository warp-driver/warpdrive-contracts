# syntax=docker/dockerfile:1.7

# Build context is the repository root; invoke as:
#   docker build -t warpdrive-stellar-middleware:dev -f docker/middleware/Dockerfile .
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
# compile its hidapi/dbus dependencies from source
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

RUN cargo install cargo-chef --locked

# Planner: extracts the dep graph into recipe.json
FROM base AS planner
WORKDIR /src
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# Cacher: pre-builds the dep tree to wasm32v1-none
FROM base AS cacher
WORKDIR /src
COPY --from=planner /src/recipe.json recipe.json
RUN cargo chef cook --release --target wasm32v1-none --recipe-path recipe.json

# Builder: copy the warm target/ first, then full source on top.
FROM base AS builder
WORKDIR /src
COPY --from=cacher /src/target target
COPY . .
RUN task optimize

# Runtime stage: slim image carrying only what the CLI needs at invocation time.
FROM debian:bookworm-slim AS runtime

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        ca-certificates \
        curl \
        jq \
        libdbus-1-3 \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /usr/local/bin/stellar /usr/local/bin/stellar

RUN mkdir -p /warpdrive/wasm
COPY --from=builder /src/target/wasm32v1-none/release/warpdrive_secp256k1_security.wasm /warpdrive/wasm/
COPY --from=builder /src/target/wasm32v1-none/release/warpdrive_secp256k1_verification.wasm /warpdrive/wasm/
COPY --from=builder /src/target/wasm32v1-none/release/warpdrive_ethereum_handler.wasm /warpdrive/wasm/
COPY --from=builder /src/target/wasm32v1-none/release/warpdrive_ed25519_security.wasm /warpdrive/wasm/
COPY --from=builder /src/target/wasm32v1-none/release/warpdrive_ed25519_verification.wasm /warpdrive/wasm/
COPY --from=builder /src/target/wasm32v1-none/release/warpdrive_stellar_handler.wasm /warpdrive/wasm/
COPY --from=builder /src/target/wasm32v1-none/release/warpdrive_project_root.wasm /warpdrive/wasm/

COPY docker/middleware/cli.sh /warpdrive/cli.sh
COPY docker/middleware/deploy.sh /warpdrive/deploy.sh
COPY docker/middleware/signers.sh /warpdrive/signers.sh
COPY docker/middleware/project_root.sh /warpdrive/project_root.sh
COPY docker/middleware/common.sh /warpdrive/common.sh
RUN chmod +x /warpdrive/*.sh

WORKDIR /warpdrive
# Long-lived container; callers issue commands via `docker exec <id> /warpdrive/cli.sh ...`.
CMD ["tail", "-f", "/dev/null"]
