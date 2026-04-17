#!/usr/bin/env bash
# Entrypoint dispatcher for the Warpdrive Stellar middleware image.

set -euo pipefail

# shellcheck source=common.sh
. /warpdrive/common.sh

usage() {
    cat <<'EOF'
usage: cli.sh <subcommand> [args]

subcommands:
  deploy         --output-path <file>
                 Deploy all 7 Soroban contracts to the configured network.
                 Env: RPC_URL, NETWORK_PASSPHRASE, DEPLOYER_SECRET,
                      [PROJECT_SPEC_REPO]

  add-signer     --scheme <secp256k1|ed25519> --key <hex>
                 --weight <u32> --deploy-file <path>
                 Register or update a signer on the matching security contract.
                 Env: RPC_URL, NETWORK_PASSPHRASE, DEPLOYER_SECRET

  remove-signer  --scheme <secp256k1|ed25519> --key <hex>
                 --deploy-file <path>

  set-threshold  --scheme <secp256k1|ed25519>
                 --numerator <u32> --denominator <u32>
                 --deploy-file <path>

  get-ledger     Print the current ledger sequence from the configured RPC.
                 Env: RPC_URL

  help           Print this message.
EOF
}

if [ "$#" -lt 1 ]; then
    usage >&2
    exit 2
fi

cmd="$1"
shift

case "$cmd" in
    deploy)
        exec /warpdrive/deploy.sh "$@"
        ;;
    add-signer|remove-signer|set-threshold)
        exec /warpdrive/signers.sh "$cmd" "$@"
        ;;
    get-ledger)
        get_latest_ledger
        ;;
    help|-h|--help)
        usage
        ;;
    *)
        echo "unknown subcommand: $cmd" >&2
        usage >&2
        exit 2
        ;;
esac
