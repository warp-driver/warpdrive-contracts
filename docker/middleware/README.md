# warpdrive-stellar-middleware

Docker image that packages the Warpdrive Soroban contracts behind a small CLI,
so Warpdrive's e2e harness can deploy and manage them the same way it does
`wavs-middleware` (Eigenlayer), `poa-middleware`, and `cw-middleware` (Cosmos).

## Build (Optional)

Build context must be the repository root:

```bash
docker build -t warpdrive-stellar-middleware:dev -f docker/middleware/Dockerfile .
```

The builder stage installs `stellar-cli`, compiles the seven contracts to
`wasm32v1-none`, and bakes them into the runtime image under `/warpdrive/wasm/`.

## Pull

All images can be found at GitHub Container Repo as `ghcr.io/warp-driver/warpdrive-stellar-middleware`.
The [CI builds](../../.github/workflows/middleware-image.yml) images on git tags, pushes to main, and PRs that modify this directory. 
You can reference the following tags:

* `latest` - last commit on `main` branch
* `0.2.0` - exact match in `v0.2.0` tag
* `0.2` - most recent patch release, could be `0.2.0`, `0.2.1`, `0.2.2`, etc
* `pr-36` - if a PR touches the docker build system, it will get tagged on the PR number 
* `13bbffc` - you can use a short git hash to refer to a commit that triggered CI. It will also be tagged with one or more of the above.

Generally, pull `ghcr.io/warp-driver/warpdrive-stellar-middleware:latest` for development and testing and
`ghcr.io/warp-driver/warpdrive-stellar-middleware:0.2.1` or similar for reproducable builds on a tagged version. 

## Run

Start a long-lived container and issue commands via `docker exec`. The
recommended local workflow is against [Stellar Quickstart](https://developers.stellar.org/docs/tools/quickstart) —
a single sidecar container that bundles Core, Soroban RPC, and friendbot.
Testnet/mainnet are documented below for PR-gated and release-gated runs.

### Local Quickstart (default)

Quickstart and the middleware run as siblings on a shared docker network so
the middleware can resolve `stellar` by name. `smoke.sh` (see below) wraps
this for you; the manual flow is:

```bash
docker network create wdnet

docker run -d --rm --name stellar --network wdnet -p 8000:8000 \
  stellar/quickstart:latest --local
# wait ~30-60s for the RPC to come up; poll http://localhost:8000/rpc

docker run -d --rm --name wdm --network wdnet \
  --pull=always \
  -e RPC_URL=http://stellar:8000/rpc \
  -e NETWORK_PASSPHRASE="Standalone Network ; February 2017" \
  -e FRIENDBOT_URL=http://stellar:8000/friendbot \
  -v $PWD/out:/out \
  ghcr.io/warp-driver/warpdrive-stellar-middleware:latest
```

Tear down with `docker rm -f wdm stellar && docker network rm wdnet`.

### Testnet / futurenet (managed)

Container generates + friendbot-funds a throwaway identity on first use.
No sidecar, no shared network:

```bash
docker run -d --rm --name wdm \
  --pull=always \
  -e RPC_URL=https://soroban-testnet.stellar.org \
  -e NETWORK_PASSPHRASE="Test SDF Network ; September 2015" \
  -v $PWD/out:/out \
  ghcr.io/warp-driver/warpdrive-stellar-middleware:latest
```

### Mainnet / BYOK

Friendbot doesn't exist on mainnet, so you must bring a funded key.
Provide the secret and its G... address; the secret is used as `--source`
directly (no identity import):

```bash
docker run -d --rm --name wdm \
  --pull=always \
  -e RPC_URL=https://soroban.stellar.org:443 \
  -e NETWORK_PASSPHRASE="Public Global Stellar Network ; September 2015" \
  -e DEPLOYER_SECRET="S..." \
  -e DEPLOYER_ADDRESS="G..." \
  -v $PWD/out:/out \
  ghcr.io/warp-driver/warpdrive-stellar-middleware:latest
```

## CLI

| Subcommand | Args | Effect |
|---|---|---|
| `deploy` | `--output-path <file>` | Deploys all 7 contracts; writes a JSON manifest to `<file>`. |
| `add-signer` | `--scheme {secp256k1,ed25519} --key <hex> --weight <u32> --deploy-file <path>` | Registers (or updates) a signer on the matching security contract. |
| `remove-signer` | `--scheme ... --key <hex> --deploy-file <path>` | Removes a signer. |
| `set-threshold` | `--scheme ... --numerator <u32> --denominator <u32> --deploy-file <path>` | Sets the consensus fraction. |
| `get-project-spec-repo` | `--deploy-file <path>` | Reads the project specification URL from the project-root contract. |
| `set-project-spec-repo` | `--repo <url> --deploy-file <path>` | Updates the project specification URL on the project-root contract (admin-only). |
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

docker exec wdm /warpdrive/cli.sh get-project-spec-repo \
  --deploy-file /out/deploy.json

docker exec wdm /warpdrive/cli.sh set-project-spec-repo \
  --repo "ipfs://bafy.../spec.json" \
  --deploy-file /out/deploy.json
```

## Environment

| Var | Required by | Notes |
|---|---|---|
| `RPC_URL` | all subcommands that hit the network | e.g. `http://stellar:8000/rpc` (local Quickstart) or `https://soroban-testnet.stellar.org` (testnet) |
| `NETWORK_PASSPHRASE` | deploy / signers | e.g. `Standalone Network ; February 2017` (local Quickstart) or `Test SDF Network ; September 2015` (testnet) |
| `DEPLOYER_SECRET` | deploy / signers (BYOK) | Stellar secret seed (`S...`). If unset, container generates + friendbot-funds one on `$FUND_NETWORK`. |
| `DEPLOYER_ADDRESS` | deploy / signers (BYOK) | Required if `DEPLOYER_SECRET` is set. The G... address matching the secret. |
| `FRIENDBOT_URL` | local Quickstart (optional) | Friendbot endpoint used to fund the generated identity. Set this to point at Quickstart's friendbot (e.g. `http://stellar:8000/friendbot`); leave unset for testnet/futurenet (uses `$FUND_NETWORK` alias instead). |
| `FUND_NETWORK` | managed mode (optional) | stellar-cli network alias used for `keys generate --fund`. Default `testnet`. Ignored when BYOK or when `FRIENDBOT_URL` is set. |
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

## Smoke testing

`smoke.sh` is a host-side wrapper that mounts `./out/` at `/out` and persists
the generated identity under `./out/.keys/` so the same admin is reused across
`docker run --rm` invocations. Run it from the repository root.

By default it runs against a local Stellar Quickstart sidecar (auto-started
on docker network `wdnet`); pass `--network testnet` as the first arg to
hit testnet instead. Use `./docker/middleware/smoke.sh down` to tear the
local sidecar back down — host bind mounts (`./out/`, `./out/.keys/`) are
preserved.

### 1. Deploy

```bash
./docker/middleware/smoke.sh deploy --output-path /out/deploy.json
jq . out/deploy.json
```

First run starts Quickstart, generates + friendbot-funds `warpdrive-deployer`
locally, and deploys the 7 contracts. Subsequent calls reuse both the
running Quickstart container and the identity. Expect 7 contract IDs plus
`admin`, `rpc_url`, `network_passphrase`.

For testnet:

```bash
./docker/middleware/smoke.sh --network testnet deploy --output-path /out/deploy.json
```

### 2. Ledger probe

```bash
./docker/middleware/smoke.sh get-ledger
```

Should print the current ledger sequence (a decimal integer). Confirms RPC
connectivity without touching the deployed contracts.

### 3. Signer ops round-trip

Uses real keypairs from the repo's `test-vectors` helper so the bytes are valid:

```bash
eval "$(cargo run -p test-vectors 2>/dev/null)"

./docker/middleware/smoke.sh add-signer \
  --scheme secp256k1 --key "$SIGNER1_PUBKEY" --weight 100 \
  --deploy-file /out/deploy.json

./docker/middleware/smoke.sh add-signer \
  --scheme ed25519 --key "$ED_SIGNER1_PUBKEY" --weight 100 \
  --deploy-file /out/deploy.json

./docker/middleware/smoke.sh set-threshold \
  --scheme secp256k1 --numerator 1 --denominator 2 \
  --deploy-file /out/deploy.json

./docker/middleware/smoke.sh remove-signer \
  --scheme secp256k1 --key "$SIGNER1_PUBKEY" \
  --deploy-file /out/deploy.json
```

Each call should print a transaction hash with no error.

### 4. Cross-check admin against a deployed contract

Read `rpc_url` and `network_passphrase` from the deploy manifest so the
same command works for local Quickstart and testnet alike. In local mode
the middleware container needs to share `wdnet` to resolve `stellar:8000`:

```bash
PROJECT_ROOT=$(jq -r .contracts.project_root out/deploy.json)
RPC_URL=$(jq -r .rpc_url out/deploy.json)
NETWORK_PASSPHRASE=$(jq -r .network_passphrase out/deploy.json)
NETWORK_ARG=()
[[ "$RPC_URL" == *"stellar:"* ]] && NETWORK_ARG=(--network wdnet)

docker run --rm \
  --pull=always \
  "${NETWORK_ARG[@]}" \
  -v $PWD/out/.keys:/root/.config/soroban \
  ghcr.io/warp-driver/warpdrive-stellar-middleware:latest \
  stellar contract invoke \
    --id "$PROJECT_ROOT" \
    --source warpdrive-deployer \
    --rpc-url "$RPC_URL" \
    --network-passphrase "$NETWORK_PASSPHRASE" \
    --send no \
    -- admin
```

Should print the same G... as `jq -r .admin out/deploy.json`.

### 5. Tear down (local mode)

```bash
./docker/middleware/smoke.sh down
```

Stops the Quickstart sidecar and removes `wdnet`. Does not touch
`./out/` so the next `smoke.sh deploy` reuses the same identity (against
a fresh Quickstart, which means previously deployed contract IDs in
`out/deploy.json` will be invalid — re-run `deploy` to refresh).
