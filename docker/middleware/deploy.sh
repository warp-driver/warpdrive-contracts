#!/usr/bin/env bash
# Deploy all 7 Warpdrive Soroban contracts and emit a JSON manifest.
# Ported from taskfiles/testnet.yml:24 (testnet:deploy).

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

require_env RPC_URL NETWORK_PASSPHRASE DEPLOYER_SECRET
ensure_deployer_key

PROJECT_SPEC_REPO="${PROJECT_SPEC_REPO:-https://github.com/warp-driver/warpdrive-contracts}"
SECP_THRESHOLD_NUM="${SECP_THRESHOLD_NUM:-2}"
SECP_THRESHOLD_DEN="${SECP_THRESHOLD_DEN:-3}"
ED_THRESHOLD_NUM="${ED_THRESHOLD_NUM:-2}"
ED_THRESHOLD_DEN="${ED_THRESHOLD_DEN:-3}"
VERIFICATION_TYPE="${VERIFICATION_TYPE:-1}"  # 1 = Ethereum

ADMIN=$(stellar keys address "$KEY_ALIAS")
echo "deploying as admin: $ADMIN" >&2

# shellcheck disable=SC2086  # word-splitting is intentional for flag expansion
NET_FLAGS=$(stellar_network_flags)

deploy_contract() {
    local wasm="$1"
    shift
    # shellcheck disable=SC2086
    retry stellar contract deploy \
        --wasm "$WASM_DIR/$wasm" \
        --source "$KEY_ALIAS" \
        $NET_FLAGS \
        --inclusion-fee "$INCLUSION_FEE" \
        -- \
        "$@"
}

echo "=== deploying secp256k1-security ===" >&2
SECP_SECURITY_ID=$(deploy_contract warpdrive_secp256k1_security.wasm \
    --admin "$ADMIN" \
    --threshold_numerator "$SECP_THRESHOLD_NUM" \
    --threshold_denominator "$SECP_THRESHOLD_DEN")
echo "secp256k1-security: $SECP_SECURITY_ID" >&2

echo "=== deploying secp256k1-verification ===" >&2
SECP_VERIFICATION_ID=$(deploy_contract warpdrive_secp256k1_verification.wasm \
    --admin "$ADMIN" \
    --security_contract "$SECP_SECURITY_ID")
echo "secp256k1-verification: $SECP_VERIFICATION_ID" >&2

echo "=== deploying ethereum-handler ===" >&2
ETH_HANDLER_ID=$(deploy_contract warpdrive_ethereum_handler.wasm \
    --admin "$ADMIN" \
    --verification_contract "$SECP_VERIFICATION_ID")
echo "ethereum-handler: $ETH_HANDLER_ID" >&2

echo "=== deploying ed25519-security ===" >&2
ED_SECURITY_ID=$(deploy_contract warpdrive_ed25519_security.wasm \
    --admin "$ADMIN" \
    --threshold_numerator "$ED_THRESHOLD_NUM" \
    --threshold_denominator "$ED_THRESHOLD_DEN")
echo "ed25519-security: $ED_SECURITY_ID" >&2

echo "=== deploying ed25519-verification ===" >&2
ED_VERIFICATION_ID=$(deploy_contract warpdrive_ed25519_verification.wasm \
    --admin "$ADMIN" \
    --security_contract "$ED_SECURITY_ID")
echo "ed25519-verification: $ED_VERIFICATION_ID" >&2

echo "=== deploying stellar-handler ===" >&2
XLM_HANDLER_ID=$(deploy_contract warpdrive_stellar_handler.wasm \
    --admin "$ADMIN" \
    --verification_contract "$ED_VERIFICATION_ID")
echo "stellar-handler: $XLM_HANDLER_ID" >&2

echo "=== deploying project-root ===" >&2
PROJECT_ROOT_ID=$(deploy_contract warpdrive_project_root.wasm \
    --admin "$ADMIN" \
    --security_contract "$SECP_SECURITY_ID" \
    --verification_contract "$SECP_VERIFICATION_ID" \
    --project_spec_repo "$PROJECT_SPEC_REPO" \
    --verification_type "$VERIFICATION_TYPE")
echo "project-root: $PROJECT_ROOT_ID" >&2

mkdir -p "$(dirname "$OUTPUT_PATH")"
jq -n \
    --arg admin "$ADMIN" \
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
        contracts: {
            secp256k1_security: $secp_security,
            secp256k1_verification: $secp_verification,
            ethereum_handler: $eth_handler,
            ed25519_security: $ed_security,
            ed25519_verification: $ed_verification,
            stellar_handler: $xlm_handler,
            project_root: $project_root
        }
    }' \
    > "$OUTPUT_PATH"

echo "wrote deployment manifest to $OUTPUT_PATH" >&2
