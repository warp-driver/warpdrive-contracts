#!/usr/bin/env bash
# Project-root operations: get-project-spec-repo, set-project-spec-repo.

set -euo pipefail

# shellcheck source=common.sh
. /warpdrive/common.sh

ACTION="${1:-}"
[ -n "$ACTION" ] || die "internal: missing action"
shift

REPO=""
DEPLOY_FILE=""

while [ "$#" -gt 0 ]; do
    case "$1" in
        --repo)        REPO="${2:-}";        shift 2 ;;
        --deploy-file) DEPLOY_FILE="${2:-}"; shift 2 ;;
        *) die "unknown argument: $1" ;;
    esac
done

require_env RPC_URL NETWORK_PASSPHRASE
resolve_deployer

[ -n "$DEPLOY_FILE" ] || die "--deploy-file is required"
[ -r "$DEPLOY_FILE" ] || die "deploy file not readable: $DEPLOY_FILE"

PROJECT_ROOT_ID=$(jq -er '.contracts.project_root' "$DEPLOY_FILE")

set_net_flags

case "$ACTION" in
    get-project-spec-repo)
        retry stellar contract invoke \
            --id "$PROJECT_ROOT_ID" \
            --source "$DEPLOY_SOURCE" \
            "${NET_FLAGS[@]}" \
            --send no \
            -- \
            project_spec_repo
        ;;
    set-project-spec-repo)
        [ -n "$REPO" ] || die "--repo is required"
        retry stellar contract invoke \
            --id "$PROJECT_ROOT_ID" \
            --source "$DEPLOY_SOURCE" \
            "${NET_FLAGS[@]}" \
            --inclusion-fee "$INCLUSION_FEE" \
            -- \
            update_project_spec_repo --repo "$REPO"
        ;;
    *)
        die "unknown action: $ACTION"
        ;;
esac
