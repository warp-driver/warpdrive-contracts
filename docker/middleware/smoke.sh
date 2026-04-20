#!/usr/bin/env bash
# Host-side convenience wrapper for smoke-testing the middleware image
# against testnet. Forwards arbitrary `cli.sh` arguments to the container
# and persists the generated identity + output JSON across invocations.
#
# First run generates + friendbot-funds a throwaway identity inside the
# container. Subsequent runs reuse it via the mounted identity dir.
#
# Usage:
#   ./smoke.sh deploy --output-path /out/deploy.json
#   ./smoke.sh get-ledger
#   ./smoke.sh add-signer --scheme secp256k1 --key 0x... --weight 100 \
#                          --deploy-file /out/deploy.json
#
# Env overrides (all optional):
#   OUT_DIR              host dir mounted at /out (default: ./out)
#   KEYS_DIR             host dir mounted at /root/.config/soroban (default: ./out/.keys)
#   IMAGE                docker image (default: warpdrive-stellar-middleware:dev)
#   RPC_URL              default: https://soroban-testnet.stellar.org
#   NETWORK_PASSPHRASE   default: "Test SDF Network ; September 2015"

set -euo pipefail

OUT_DIR="${OUT_DIR:-$(pwd)/out}"
KEYS_DIR="${KEYS_DIR:-$OUT_DIR/.keys}"
IMAGE="${IMAGE:-warpdrive-stellar-middleware:dev}"
export RPC_URL="${RPC_URL:-https://soroban-testnet.stellar.org}"
export NETWORK_PASSPHRASE="${NETWORK_PASSPHRASE:-Test SDF Network ; September 2015}"

mkdir -p "$OUT_DIR" "$KEYS_DIR"
OUT_DIR=$(cd "$OUT_DIR" && pwd)
KEYS_DIR=$(cd "$KEYS_DIR" && pwd)

exec docker run --rm \
    -e RPC_URL \
    -e NETWORK_PASSPHRASE \
    -v "$OUT_DIR":/out \
    -v "$KEYS_DIR":/root/.config/soroban \
    "$IMAGE" \
    /warpdrive/cli.sh "$@"
