#!/usr/bin/env bash
# Shared helpers sourced by cli.sh / deploy.sh / signers.sh.

set -euo pipefail

WASM_DIR="${WASM_DIR:-/warpdrive/wasm}"
KEY_ALIAS="${KEY_ALIAS:-warpdrive-deployer}"
MAX_RETRIES="${MAX_RETRIES:-3}"
RETRY_SLEEP_SECONDS="${RETRY_SLEEP_SECONDS:-5}"
INCLUSION_FEE="${INCLUSION_FEE:-10000000}"

die() {
    echo "error: $*" >&2
    exit 1
}

require_env() {
    local name
    for name in "$@"; do
        if [ -z "${!name:-}" ]; then
            die "required env var $name is not set"
        fi
    done
}

retry() {
    local attempt=1
    while [ "$attempt" -le "$MAX_RETRIES" ]; do
        if "$@"; then
            return 0
        fi
        echo "  attempt $attempt/$MAX_RETRIES failed, retrying in ${RETRY_SLEEP_SECONDS}s..." >&2
        sleep "$RETRY_SLEEP_SECONDS"
        attempt=$((attempt + 1))
    done
    return 1
}

# Imports $DEPLOYER_SECRET into stellar-cli under $KEY_ALIAS.
# Idempotent: safe to call multiple times in the same container.
ensure_deployer_key() {
    require_env DEPLOYER_SECRET
    if stellar keys address "$KEY_ALIAS" >/dev/null 2>&1; then
        return 0
    fi
    stellar keys add "$KEY_ALIAS" --secret-key <<<"$DEPLOYER_SECRET" >/dev/null
}

# Echoes the stellar network flags common to deploy + invoke.
stellar_network_flags() {
    require_env RPC_URL NETWORK_PASSPHRASE
    printf -- '--rpc-url %s --network-passphrase %s' \
        "$RPC_URL" "$NETWORK_PASSPHRASE"
}

# Fetches current ledger sequence from RPC.
get_latest_ledger() {
    require_env RPC_URL
    curl -sS -X POST "$RPC_URL" \
        -H 'Content-Type: application/json' \
        -d '{"jsonrpc":"2.0","id":1,"method":"getLatestLedger"}' \
        | jq -r '.result.sequence'
}
