# warpdrive-stellar-middleware

Docker image that packages the Warpdrive Soroban contracts behind a small CLI,
so Warpdrive's e2e harness can deploy and manage them the same way it does
`wavs-middleware` (Eigenlayer), `poa-middleware`, and `cw-middleware` (Cosmos).

## Build

Build context must be the repository root:

```bash
docker build -t warpdrive-stellar-middleware:dev -f docker/middleware/Dockerfile .
```

The builder stage installs `stellar-cli`, compiles the seven contracts to
`wasm32v1-none`, and bakes them into the runtime image under `/warpdrive/wasm/`.

## Run

Start a long-lived container and issue commands via `docker exec`.

**Testnet / futurenet (managed)** — container generates + friendbot-funds a
throwaway identity on first use:

```bash
docker run -d --name wdm \
  -e RPC_URL=https://soroban-testnet.stellar.org \
  -e NETWORK_PASSPHRASE="Test SDF Network ; September 2015" \
  -v $PWD/out:/out \
  warpdrive-stellar-middleware:dev
```

**Mainnet / BYOK** — friendbot doesn't exist, so you must bring a funded key.
Provide the secret and its G... address; the secret is used as `--source`
directly (no identity import):

```bash
docker run -d --name wdm \
  -e RPC_URL=https://soroban.stellar.org:443 \
  -e NETWORK_PASSPHRASE="Public Global Stellar Network ; September 2015" \
  -e DEPLOYER_SECRET="S..." \
  -e DEPLOYER_ADDRESS="G..." \
  -v $PWD/out:/out \
  warpdrive-stellar-middleware:dev
```

## CLI

| Subcommand | Args | Effect |
|---|---|---|
| `deploy` | `--output-path <file>` | Deploys all 7 contracts; writes a JSON manifest to `<file>`. |
| `add-signer` | `--scheme {secp256k1,ed25519} --key <hex> --weight <u32> --deploy-file <path>` | Registers (or updates) a signer on the matching security contract. |
| `remove-signer` | `--scheme ... --key <hex> --deploy-file <path>` | Removes a signer. |
| `set-threshold` | `--scheme ... --numerator <u32> --denominator <u32> --deploy-file <path>` | Sets the consensus fraction. |
| `get-ledger` | — | Prints the current ledger sequence (for `reference_block` lookups). |
| `help` | — | Prints usage. |

### Example

```bash
docker exec wdm /warpdrive/cli.sh deploy --output-path /out/deploy.json

docker exec wdm /warpdrive/cli.sh add-signer \
  --scheme secp256k1 --key 0xabcd... --weight 100 \
  --deploy-file /out/deploy.json

docker exec wdm /warpdrive/cli.sh set-threshold \
  --scheme secp256k1 --numerator 2 --denominator 3 \
  --deploy-file /out/deploy.json
```

## Environment

| Var | Required by | Notes |
|---|---|---|
| `RPC_URL` | all subcommands that hit the network | e.g. `https://soroban-testnet.stellar.org` |
| `NETWORK_PASSPHRASE` | deploy / signers | e.g. `Test SDF Network ; September 2015` |
| `DEPLOYER_SECRET` | deploy / signers (BYOK) | Stellar secret seed (`S...`). If unset, container generates + friendbot-funds one on `$FUND_NETWORK`. |
| `DEPLOYER_ADDRESS` | deploy / signers (BYOK) | Required if `DEPLOYER_SECRET` is set. The G... address matching the secret. |
| `FUND_NETWORK` | managed mode (optional) | stellar-cli network alias used for `keys generate --fund`. Default `testnet`. Ignored when BYOK. |
| `KEY_ALIAS` | managed mode (optional) | stellar-cli identity alias for the generated key. Default `warpdrive-deployer`. |
| `PROJECT_SPEC_REPO` | deploy (optional) | URL written into project-root at init. Default: warp-driver/warpdrive-contracts. |
| `SECP_THRESHOLD_NUM` / `SECP_THRESHOLD_DEN` | deploy (optional) | Initial threshold on secp256k1 security. Default `2/3`. |
| `ED_THRESHOLD_NUM` / `ED_THRESHOLD_DEN` | deploy (optional) | Initial threshold on ed25519 security. Default `2/3`. |
| `VERIFICATION_TYPE` | deploy (optional) | Enum value written into project-root. Default `1` (Ethereum). |
| `INCLUSION_FEE` | all invocations | Default `10000000` stroops. |
| `MAX_RETRIES` / `RETRY_SLEEP_SECONDS` | all invocations | Retry config for RPC hiccups. Default `3` / `5`. |

## Output manifest

`deploy` writes:

```json
{
  "admin": "G...",
  "rpc_url": "...",
  "network_passphrase": "...",
  "contracts": {
    "secp256k1_security": "C...",
    "secp256k1_verification": "C...",
    "ethereum_handler": "C...",
    "ed25519_security": "C...",
    "ed25519_verification": "C...",
    "stellar_handler": "C...",
    "project_root": "C..."
  }
}
```
