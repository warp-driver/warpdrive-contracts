#!/usr/bin/env bash
# Host-side wrapper for smoke-testing the middleware image.
#
# Defaults to testnet (https://soroban-testnet.stellar.org). Pass
# `--network local` as the first arg to spin up a local Stellar Quickstart
# sidecar on docker network `wdnet` — useful for offline iteration but
# slower to bootstrap.
#
# Identity: the deployer's keyfile lives at /out/.keys/deployer.secret inside
# the mounted OUT_DIR, so it persists across invocations. Before any command
# that needs to sign, this wrapper runs `keygen` once (idempotent) to ensure a
# funded identity exists; the actual command then reads that keyfile via the
# default-keyfile precedence.
#
# Usage:
#   ./smoke.sh deploy --output-path /out/deploy.json
#   ./smoke.sh deploy --output-path /out/deploy-stellar.json --variant stellar
#   ./smoke.sh get-ledger
#   ./smoke.sh add-signer --scheme secp256k1 --key 0x... --weight 100 \
#                          --deploy-file /out/deploy.json
#   ./smoke.sh --network local deploy --output-path /out/deploy.json
#   ./smoke.sh down       # tear down the local Quickstart sidecar + network
#
# Env overrides (all optional):
#   OUT_DIR              host dir mounted at /out (default: ./out)
#   IMAGE                middleware image (default: warpdrive-stellar-middleware:dev)
#   QUICKSTART_IMAGE     local mode only; overrides the pinned default
#   QUICKSTART_NAME      local mode only (default: stellar)
#   DOCKER_NETWORK       local mode only (default: wdnet)
#   QUICKSTART_TIMEOUT   seconds to wait for Quickstart RPC (default: 180)
#   RPC_URL              override the default for the chosen network
#   NETWORK_PASSPHRASE   override the default for the chosen network
#   FRIENDBOT_URL        override the default for the chosen network

set -euo pipefail

NETWORK="testnet"
if [ "${1:-}" = "--network" ]; then
    NETWORK="${2:-}"
    shift 2
fi

case "$NETWORK" in
    local|testnet) ;;
    *) echo "smoke.sh: --network must be 'local' or 'testnet' (got: $NETWORK)" >&2; exit 2 ;;
esac

OUT_DIR="${OUT_DIR:-$(pwd)/out}"
IMAGE="${IMAGE:-warpdrive-stellar-middleware:dev}"
QUICKSTART_IMAGE="${QUICKSTART_IMAGE:-stellar/quickstart:v637-b1021.1-latest}"
QUICKSTART_NAME="${QUICKSTART_NAME:-stellar}"
DOCKER_NETWORK="${DOCKER_NETWORK:-wdnet}"
QUICKSTART_TIMEOUT="${QUICKSTART_TIMEOUT:-180}"

down() {
    if docker ps -a --format '{{.Names}}' | grep -qx "$QUICKSTART_NAME"; then
        echo "==> stopping $QUICKSTART_NAME" >&2
        docker rm -f "$QUICKSTART_NAME" >/dev/null
    fi
    if docker network inspect "$DOCKER_NETWORK" >/dev/null 2>&1; then
        echo "==> removing network $DOCKER_NETWORK" >&2
        docker network rm "$DOCKER_NETWORK" >/dev/null
    fi
}

if [ "${1:-}" = "down" ]; then
    down
    exit 0
fi

mkdir -p "$OUT_DIR"
OUT_DIR=$(cd "$OUT_DIR" && pwd)

if [ "$NETWORK" = "local" ]; then
    export RPC_URL="${RPC_URL:-http://${QUICKSTART_NAME}:8000/rpc}"
    export NETWORK_PASSPHRASE="${NETWORK_PASSPHRASE:-Standalone Network ; February 2017}"
    export FRIENDBOT_URL="${FRIENDBOT_URL:-http://${QUICKSTART_NAME}:8000/friendbot}"

    if ! docker network inspect "$DOCKER_NETWORK" >/dev/null 2>&1; then
        echo "==> creating docker network $DOCKER_NETWORK" >&2
        docker network create "$DOCKER_NETWORK" >/dev/null
    fi

    if ! docker ps --format '{{.Names}}' | grep -qx "$QUICKSTART_NAME"; then
        echo "==> starting $QUICKSTART_IMAGE as '$QUICKSTART_NAME' on $DOCKER_NETWORK" >&2
        docker run -d --rm \
            --name "$QUICKSTART_NAME" \
            --network "$DOCKER_NETWORK" \
            -p 8000:8000 \
            "$QUICKSTART_IMAGE" --local >/dev/null
    fi

    echo "==> waiting for Quickstart RPC + friendbot (up to ${QUICKSTART_TIMEOUT}s)" >&2
    deadline=$(( $(date +%s) + QUICKSTART_TIMEOUT ))
    seq=""
    rpc_ready=0
    friendbot_ready=0
    while :; do
        # `|| true` prevents `set -e` / pipefail from killing the script while
        # the upstream services aren't reachable yet (curl 7, 22, 28 etc).
        if [ "$rpc_ready" = 0 ]; then
            body=$(curl -fsS -X POST http://localhost:8000/rpc \
                -H 'Content-Type: application/json' \
                -d '{"jsonrpc":"2.0","id":1,"method":"getLatestLedger"}' 2>/dev/null || true)
            if [ -n "$body" ]; then
                seq=$(printf '%s' "$body" | jq -r '.result.sequence // empty' 2>/dev/null || true)
                case "$seq" in
                    ''|null|*[!0-9]*) ;;
                    *) [ "$seq" -gt 0 ] && rpc_ready=1 ;;
                esac
            fi
        fi
        if [ "$friendbot_ready" = 0 ]; then
            http_code=$(curl -s -o /dev/null -w '%{http_code}' "http://localhost:8000/friendbot" 2>/dev/null || true)
            case "$http_code" in
                ''|000|5*) ;;  # not up / proxy error
                *) friendbot_ready=1 ;;
            esac
        fi
        if [ "$rpc_ready" = 1 ] && [ "$friendbot_ready" = 1 ]; then
            break
        fi
        if [ "$(date +%s)" -ge "$deadline" ]; then
            echo "Quickstart did not become fully ready within ${QUICKSTART_TIMEOUT}s" >&2
            echo "  rpc_ready=$rpc_ready friendbot_ready=$friendbot_ready" >&2
            echo "  last RPC body: ${body:-<empty>}" >&2
            echo "  last friendbot HTTP: ${http_code:-<empty>}" >&2
            echo "  container status: $(docker ps -a --filter "name=^${QUICKSTART_NAME}$" --format '{{.Status}}')" >&2
            exit 1
        fi
        sleep 2
    done
    echo "==> Quickstart ready (ledger $seq, friendbot up)" >&2

    DOCKER_NET_ARGS=(--network "$DOCKER_NETWORK")
    DOCKER_ENV_ARGS=(-e RPC_URL -e NETWORK_PASSPHRASE -e FRIENDBOT_URL)
else
    export RPC_URL="${RPC_URL:-https://soroban-testnet.stellar.org}"
    export NETWORK_PASSPHRASE="${NETWORK_PASSPHRASE:-Test SDF Network ; September 2015}"
    DOCKER_NET_ARGS=()
    DOCKER_ENV_ARGS=(-e RPC_URL -e NETWORK_PASSPHRASE)
    [ -n "${FRIENDBOT_URL:-}" ] && DOCKER_ENV_ARGS+=(-e FRIENDBOT_URL)
fi

run_deployer() {
    docker run --rm \
        "${DOCKER_NET_ARGS[@]}" \
        "${DOCKER_ENV_ARGS[@]}" \
        -v "$OUT_DIR":/out \
        "$IMAGE" \
        warpdrive-deployer "$@"
}

# Ensure a funded identity exists (idempotent) before any command that signs.
cmd="${1:-}"
case "$cmd" in
    get-ledger|keygen|help|--help|-h|"") ;;
    *)
        echo "==> ensuring deployer identity (keygen)" >&2
        run_deployer keygen >/dev/null
        ;;
esac

exec_args=("$@")
run_deployer "${exec_args[@]}"
