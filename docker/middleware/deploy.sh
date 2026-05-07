#!/usr/bin/env bash
# Deploy all 7 Warpdrive Soroban contracts and emit a JSON manifest.
# Ported from taskfiles/testnet.yml:24 (testnet:deploy).
#
# Idempotent: each contract ID is checkpointed to OUTPUT_PATH as soon as
# it is deployed. If the script is re-run with the same --output-path
# (after a mid-script abort or after the caller retries the wrapper),
# already-deployed contracts are reused and only the remaining steps run.
# Per-step retries (via common.sh `retry`) still cap at MAX_RETRIES per
# contract; on exhaustion the script aborts without rolling back the
# checkpoint, so the next run picks up exactly where this one stopped.

set -euo pipefail

# shellcheck source=common.sh
. /warpdrive/common.sh

OUTPUT_PATH=""

while [ "$#" -gt 0 ]; do
    case "$1" in
        --output-path)
            OUTPUT_PATH="${2:-}"
            shift 2
            ;;
        *)
            die "unknown argument: $1"
            ;;
    esac
done

[ -n "$OUTPUT_PATH" ] || die "--output-path is required"

require_env RPC_URL NETWORK_PASSPHRASE
resolve_deployer

PROJECT_SPEC_REPO="${PROJECT_SPEC_REPO:-https://github.com/warp-driver/warpdrive-contracts}"
SECP_THRESHOLD_NUM="${SECP_THRESHOLD_NUM:-2}"
SECP_THRESHOLD_DEN="${SECP_THRESHOLD_DEN:-3}"
ED_THRESHOLD_NUM="${ED_THRESHOLD_NUM:-2}"
ED_THRESHOLD_DEN="${ED_THRESHOLD_DEN:-3}"
VERIFICATION_TYPE="${VERIFICATION_TYPE:-1}"  # 1 = Ethereum

echo "deploying as admin: $ADMIN_ADDRESS" >&2

set_net_flags

mkdir -p "$(dirname "$OUTPUT_PATH")"

SECP_SECURITY_ID=""
SECP_VERIFICATION_ID=""
ETH_HANDLER_ID=""
ED_SECURITY_ID=""
ED_VERIFICATION_ID=""
XLM_HANDLER_ID=""
PROJECT_ROOT_ID=""

# Restore any previously-deployed contract IDs so a re-run skips the
# steps that already succeeded. jq returns "" for missing keys.
load_existing() {
    [ -r "$OUTPUT_PATH" ] || return 0
    if ! jq -e . "$OUTPUT_PATH" >/dev/null 2>&1; then
        echo "warning: $OUTPUT_PATH is not valid JSON, ignoring" >&2
        return 0
    fi
    SECP_SECURITY_ID=$(jq -r '.contracts.secp256k1_security // ""' "$OUTPUT_PATH")
    SECP_VERIFICATION_ID=$(jq -r '.contracts.secp256k1_verification // ""' "$OUTPUT_PATH")
    ETH_HANDLER_ID=$(jq -r '.contracts.ethereum_handler // ""' "$OUTPUT_PATH")
    ED_SECURITY_ID=$(jq -r '.contracts.ed25519_security // ""' "$OUTPUT_PATH")
    ED_VERIFICATION_ID=$(jq -r '.contracts.ed25519_verification // ""' "$OUTPUT_PATH")
    XLM_HANDLER_ID=$(jq -r '.contracts.stellar_handler // ""' "$OUTPUT_PATH")
    PROJECT_ROOT_ID=$(jq -r '.contracts.project_root // ""' "$OUTPUT_PATH")
}

# Atomically rewrite OUTPUT_PATH with whatever IDs are populated. Empty
# entries are filtered out so downstream `jq -e` reads on partial files
# fail loudly rather than returning an empty string.
persist_manifest() {
    local tmp
    tmp=$(mktemp)
    jq -n \
        --arg admin "$ADMIN_ADDRESS" \
        --arg rpc_url "$RPC_URL" \
        --arg network_passphrase "$NETWORK_PASSPHRASE" \
        --arg secp_security "$SECP_SECURITY_ID" \
        --arg secp_verification "$SECP_VERIFICATION_ID" \
        --arg eth_handler "$ETH_HANDLER_ID" \
        --arg ed_security "$ED_SECURITY_ID" \
        --arg ed_verification "$ED_VERIFICATION_ID" \
        --arg xlm_handler "$XLM_HANDLER_ID" \
        --arg project_root "$PROJECT_ROOT_ID" \
        '{
            admin: $admin,
            rpc_url: $rpc_url,
            network_passphrase: $network_passphrase,
            contracts: ({
                secp256k1_security: $secp_security,
                secp256k1_verification: $secp_verification,
                ethereum_handler: $eth_handler,
                ed25519_security: $ed_security,
                ed25519_verification: $ed_verification,
                stellar_handler: $xlm_handler,
                project_root: $project_root
            } | with_entries(select(.value != "")))
        }' \
        > "$tmp"
    mv "$tmp" "$OUTPUT_PATH"
}

deploy_contract() {
    local wasm="$1"
    shift
    retry stellar contract deploy \
        --wasm "$WASM_DIR/$wasm" \
        --source "$DEPLOY_SOURCE" \
        "${NET_FLAGS[@]}" \
        --inclusion-fee "$INCLUSION_FEE" \
        -- \
        "$@"
}

# deploy_step LABEL VAR_NAME WASM [-- CTOR_ARGS...]
# Skips when $VAR_NAME is already set; otherwise deploys, assigns, and
# checkpoints the manifest. Aborts the script if the per-step retry
# budget is exhausted (so checkpoints are never written for failed
# deploys).
deploy_step() {
    local label="$1" var_name="$2" wasm="$3"
    shift 3
    local existing="${!var_name}"
    if [ -n "$existing" ]; then
        echo "=== reusing $label ($existing) ===" >&2
        return 0
    fi
    echo "=== deploying $label ===" >&2
    local id
    id=$(deploy_contract "$wasm" "$@")
    printf -v "$var_name" '%s' "$id"
    persist_manifest
    echo "$label: $id" >&2
}

load_existing

deploy_step "secp256k1-security" SECP_SECURITY_ID warpdrive_secp256k1_security.wasm \
    --admin "$ADMIN_ADDRESS" \
    --threshold_numerator "$SECP_THRESHOLD_NUM" \
    --threshold_denominator "$SECP_THRESHOLD_DEN"

deploy_step "secp256k1-verification" SECP_VERIFICATION_ID warpdrive_secp256k1_verification.wasm \
    --admin "$ADMIN_ADDRESS" \
    --security_contract "$SECP_SECURITY_ID"

deploy_step "ethereum-handler" ETH_HANDLER_ID warpdrive_ethereum_handler.wasm \
    --admin "$ADMIN_ADDRESS" \
    --verification_contract "$SECP_VERIFICATION_ID"

deploy_step "ed25519-security" ED_SECURITY_ID warpdrive_ed25519_security.wasm \
    --admin "$ADMIN_ADDRESS" \
    --threshold_numerator "$ED_THRESHOLD_NUM" \
    --threshold_denominator "$ED_THRESHOLD_DEN"

deploy_step "ed25519-verification" ED_VERIFICATION_ID warpdrive_ed25519_verification.wasm \
    --admin "$ADMIN_ADDRESS" \
    --security_contract "$ED_SECURITY_ID"

deploy_step "stellar-handler" XLM_HANDLER_ID warpdrive_stellar_handler.wasm \
    --admin "$ADMIN_ADDRESS" \
    --verification_contract "$ED_VERIFICATION_ID"

deploy_step "project-root" PROJECT_ROOT_ID warpdrive_project_root.wasm \
    --admin "$ADMIN_ADDRESS" \
    --security_contract "$SECP_SECURITY_ID" \
    --verification_contract "$SECP_VERIFICATION_ID" \
    --project_spec_repo "$PROJECT_SPEC_REPO" \
    --verification_type "$VERIFICATION_TYPE"

echo "wrote deployment manifest to $OUTPUT_PATH" >&2
