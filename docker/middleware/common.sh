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
# Three modes:
#   * BYOK (mainnet, anywhere without friendbot):
#     caller sets DEPLOYER_SECRET (S...) and DEPLOYER_ADDRESS (G...).
#     The raw secret is passed straight to stellar --source; no key import.
#   * Local Quickstart (FRIENDBOT_URL set):
#     no secret set, generates an identity and funds it via the explicit
#     friendbot URL. Used when stellar-cli has no built-in alias for the
#     network (e.g. Stellar Quickstart's Standalone Network).
#   * Managed (testnet/futurenet):
#     no secret set, no FRIENDBOT_URL. Generates + friendbot-funds $KEY_ALIAS
#     on $FUND_NETWORK using stellar-cli's built-in alias.
#
# Identity is reused across invocations as long as stellar-cli's identity
# dir persists (mount a volume to keep it across `docker run --rm` cycles).
resolve_deployer() {
    if [ -n "${DEPLOYER_SECRET:-}" ]; then
        require_env DEPLOYER_ADDRESS
        DEPLOY_SOURCE="$DEPLOYER_SECRET"
        ADMIN_ADDRESS="$DEPLOYER_ADDRESS"
        return 0
    fi
    if ! stellar keys address "$KEY_ALIAS" >/dev/null 2>&1; then
        if [ -n "${FRIENDBOT_URL:-}" ]; then
            echo "==> generating identity '$KEY_ALIAS'" >&2
            stellar keys generate "$KEY_ALIAS" >/dev/null
        else
            echo "==> generating + funding identity '$KEY_ALIAS' on '$FUND_NETWORK'" >&2
            stellar keys generate "$KEY_ALIAS" --network "$FUND_NETWORK" --fund >/dev/null
        fi
    fi
    DEPLOY_SOURCE="$KEY_ALIAS"
    ADMIN_ADDRESS=$(stellar keys address "$KEY_ALIAS")

    # In FRIENDBOT_URL (Quickstart) mode the chain state is ephemeral —
    # tearing down and recreating Quickstart leaves the host-persisted
    # identity intact but unfunded on the fresh chain. Always check the
    # account is visible on RPC and friendbot-fund it if not.
    if [ -n "${FRIENDBOT_URL:-}" ]; then
        ensure_funded "$ADMIN_ADDRESS"
    fi
}

# Funds $1 via FRIENDBOT_URL if it's not already visible to Soroban RPC.
# Idempotent — safe to call on every invocation.
ensure_funded() {
    local addr="$1"
    require_env RPC_URL FRIENDBOT_URL
    if account_visible "$addr"; then
        return 0
    fi
    echo "==> funding $addr via $FRIENDBOT_URL" >&2
    local response
    response=$(retry curl -fsS "${FRIENDBOT_URL}?addr=${addr}")
    echo "    friendbot response: ${response:0:200}" >&2
    wait_for_account "$addr"
}

# Quickstart-only: Horizon and friendbot share a host, so derive Horizon
# from FRIENDBOT_URL by stripping the /friendbot suffix.
horizon_base() {
    printf '%s' "${FRIENDBOT_URL%/friendbot}"
}

account_visible() {
    local addr="$1"
    local code
    code=$(curl -s -o /dev/null -w '%{http_code}' "$(horizon_base)/accounts/${addr}" 2>/dev/null || true)
    [ "$code" = "200" ]
}

# Polls Horizon's /accounts/{addr} until $1 is visible, or fails after
# ACCOUNT_PROPAGATE_TIMEOUT seconds (default 60). Quickstart in --local
# mode takes a couple of ledger closes (~5s each) for a friendbot fund to
# show up on Horizon. Soroban RPC reads from the same Core state, so
# Horizon visibility is a sufficient proxy for "deploys will work".
wait_for_account() {
    local addr="$1"
    local timeout="${ACCOUNT_PROPAGATE_TIMEOUT:-60}"
    local deadline=$(( $(date +%s) + timeout ))
    echo "    waiting up to ${timeout}s for $addr to appear on Horizon" >&2
    while :; do
        if account_visible "$addr"; then
            echo "    account visible" >&2
            return 0
        fi
        if [ "$(date +%s)" -ge "$deadline" ]; then
            local body
            body=$(curl -s "$(horizon_base)/accounts/${addr}" 2>/dev/null || true)
            echo "account $addr did not appear on Horizon within ${timeout}s" >&2
            echo "last Horizon body: ${body:-<empty>}" >&2
            exit 1
        fi
        sleep 2
    done
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
