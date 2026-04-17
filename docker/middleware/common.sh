#!/usr/bin/env bash
# Shared helpers sourced by cli.sh / deploy.sh / signers.sh.

set -euo pipefail

WASM_DIR="${WASM_DIR:-/warpdrive/wasm}"
KEY_ALIAS="${KEY_ALIAS:-warpdrive-deployer}"
FUND_NETWORK="${FUND_NETWORK:-testnet}"
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

# Resolves the deployer identity and sets two globals for the caller:
#   DEPLOY_SOURCE   - value passed to `stellar ... --source` (alias or raw secret)
#   ADMIN_ADDRESS   - G... used as the `--admin` arg for contract constructors
#
# Two modes:
#   * BYOK (mainnet, anywhere without friendbot):
#     caller sets DEPLOYER_SECRET (S...) and DEPLOYER_ADDRESS (G...).
#     The raw secret is passed straight to stellar --source; no key import.
#   * Managed (testnet/futurenet):
#     no secret set. Generates + friendbot-funds $KEY_ALIAS on $FUND_NETWORK
#     on first call, then reuses it. Idempotent across invocations as long as
#     stellar-cli's identity dir persists (mount a volume to keep it across
#     `docker run --rm` cycles).
resolve_deployer() {
    if [ -n "${DEPLOYER_SECRET:-}" ]; then
        require_env DEPLOYER_ADDRESS
        DEPLOY_SOURCE="$DEPLOYER_SECRET"
        ADMIN_ADDRESS="$DEPLOYER_ADDRESS"
        return 0
    fi
    if ! stellar keys address "$KEY_ALIAS" >/dev/null 2>&1; then
        echo "==> generating + funding identity '$KEY_ALIAS' on '$FUND_NETWORK'" >&2
        stellar keys generate "$KEY_ALIAS" --network "$FUND_NETWORK" --fund >/dev/null
    fi
    DEPLOY_SOURCE="$KEY_ALIAS"
    ADMIN_ADDRESS=$(stellar keys address "$KEY_ALIAS")
}

# Populates the NET_FLAGS bash array with the stellar network flags. Callers
# expand it quoted: `stellar contract deploy ... "${NET_FLAGS[@]}" ...`.
#
# Uses an array instead of a printed string so values with whitespace (such
# as NETWORK_PASSPHRASE="Test SDF Network ; September 2015") aren't split by
# word-splitting when the helper's output is re-expanded.
set_net_flags() {
    require_env RPC_URL NETWORK_PASSPHRASE
    NET_FLAGS=(--rpc-url "$RPC_URL" --network-passphrase "$NETWORK_PASSPHRASE")
}

# Fetches current ledger sequence from RPC.
get_latest_ledger() {
    require_env RPC_URL
    curl -sS -X POST "$RPC_URL" \
        -H 'Content-Type: application/json' \
        -d '{"jsonrpc":"2.0","id":1,"method":"getLatestLedger"}' \
        | jq -r '.result.sequence'
}
