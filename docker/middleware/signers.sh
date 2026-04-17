#!/usr/bin/env bash
# Signer-set management: add-signer, remove-signer, set-threshold.

set -euo pipefail

# shellcheck source=common.sh
. /warpdrive/common.sh

ACTION="${1:-}"
[ -n "$ACTION" ] || die "internal: missing action"
shift

SCHEME=""
KEY=""
WEIGHT=""
NUMERATOR=""
DENOMINATOR=""
DEPLOY_FILE=""

while [ "$#" -gt 0 ]; do
    case "$1" in
        --scheme)      SCHEME="${2:-}";      shift 2 ;;
        --key)         KEY="${2:-}";         shift 2 ;;
        --weight)      WEIGHT="${2:-}";      shift 2 ;;
        --numerator)   NUMERATOR="${2:-}";   shift 2 ;;
        --denominator) DENOMINATOR="${2:-}"; shift 2 ;;
        --deploy-file) DEPLOY_FILE="${2:-}"; shift 2 ;;
        *) die "unknown argument: $1" ;;
    esac
done

require_env RPC_URL NETWORK_PASSPHRASE
resolve_deployer

[ -n "$DEPLOY_FILE" ] || die "--deploy-file is required"
[ -r "$DEPLOY_FILE" ] || die "deploy file not readable: $DEPLOY_FILE"

case "$SCHEME" in
    secp256k1) SECURITY_ID=$(jq -er '.contracts.secp256k1_security' "$DEPLOY_FILE") ;;
    ed25519)   SECURITY_ID=$(jq -er '.contracts.ed25519_security'   "$DEPLOY_FILE") ;;
    "")        die "--scheme is required (secp256k1|ed25519)" ;;
    *)         die "invalid --scheme: $SCHEME (want secp256k1|ed25519)" ;;
esac

set_net_flags

invoke() {
    retry stellar contract invoke \
        --id "$SECURITY_ID" \
        --source "$DEPLOY_SOURCE" \
        "${NET_FLAGS[@]}" \
        --inclusion-fee "$INCLUSION_FEE" \
        -- \
        "$@"
}

case "$ACTION" in
    add-signer)
        [ -n "$KEY" ]    || die "--key is required"
        [ -n "$WEIGHT" ] || die "--weight is required"
        invoke add_signer --key "$KEY" --weight "$WEIGHT"
        ;;
    remove-signer)
        [ -n "$KEY" ] || die "--key is required"
        invoke remove_signer --key "$KEY"
        ;;
    set-threshold)
        [ -n "$NUMERATOR" ]   || die "--numerator is required"
        [ -n "$DENOMINATOR" ] || die "--denominator is required"
        invoke set_threshold \
            --numerator "$NUMERATOR" \
            --denominator "$DENOMINATOR"
        ;;
    *)
        die "unknown action: $ACTION"
        ;;
esac
