# warpdrive-stellar-middleware

Docker image that packages the Warpdrive Soroban contracts behind a single
native CLI binary (`warpdrive-deployer`), so Warpdrive's e2e harness can deploy
and manage them the same way it does `wavs-middleware` (Eigenlayer),
`poa-middleware`, and `cw-middleware` (Cosmos).

The deployer drives [`warpdrive-client`](../../packages/client) /
`wasi-soroban-rs` directly — **no shell scripts and no `stellar` CLI**. The
image carries just the binary, the contract wasm, and TLS roots.

## Build (Optional)

Build context must be the repository root:

```bash
docker build -t warpdrive-stellar-middleware:dev -f docker/middleware/Dockerfile .
```

The builder stage compiles the contracts to `wasm32v1-none` (baked into
`/warpdrive/wasm/`) **and** the native `warpdrive-deployer` binary (installed at
`/usr/local/bin/`). The deployer is its own crate, excluded from the contract
workspace, so its `clap`/`tokio` deps never touch the wasm builds.

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

Start a long-lived container and issue commands via `docker exec`. Testnet
is the default (managed Stellar testnet with built-in friendbot). A local
[Stellar Quickstart](https://developers.stellar.org/docs/tools/quickstart)
sidecar is documented below for offline iteration.

### Testnet / futurenet (default)

```bash
docker run -d --rm --name wdm \
  --pull=always \
  -e RPC_URL=https://soroban-testnet.stellar.org \
  -e NETWORK_PASSPHRASE="Test SDF Network ; September 2015" \
  -v $PWD/out:/out \
  ghcr.io/warp-driver/warpdrive-stellar-middleware:latest
```

Then create a funded identity once and deploy:

```bash
docker exec wdm warpdrive-deployer keygen          # generates + friendbot-funds /out/.keys/deployer.secret
docker exec wdm warpdrive-deployer deploy --output-path /out/deploy.json
```

`keygen` writes the secret to `/out/.keys/deployer.secret` (mode `0600`); the
other commands read it automatically via the default-keyfile precedence, so you
don't have to pass `--secret` each time.

### Local Quickstart (opt-in)

Quickstart and the middleware run as siblings on a shared docker network so
the middleware can resolve `stellar` by name. `smoke.sh --network local`
(see below) wraps this for you; for a persistent local environment, use
`docker compose`:

```bash
docker compose -f docker/middleware/docker-compose.yml up -d
# `wdm` won't start until stellar's RPC healthcheck passes.

# Tear everything down (host bind mount `./out/` is preserved):
docker compose -f docker/middleware/docker-compose.yml down
```

For local Quickstart, point `keygen` at Quickstart's friendbot via
`FRIENDBOT_URL` (the compose file already sets it):

```bash
docker exec wdm warpdrive-deployer keygen          # uses $FRIENDBOT_URL
docker exec wdm warpdrive-deployer deploy --output-path /out/deploy.json
```

### Mainnet / BYOK

Friendbot doesn't exist on mainnet, so you must bring a funded key — there's no
`keygen` step. Provide the secret; the `G…` admin address is **derived** from it
(no separate `DEPLOYER_ADDRESS`):

```bash
docker run -d --rm --name wdm \
  --pull=always \
  -e RPC_URL=https://soroban.stellar.org:443 \
  -e NETWORK_PASSPHRASE="Public Global Stellar Network ; September 2015" \
  -e DEPLOYER_SECRET="S..." \
  -v $PWD/out:/out \
  ghcr.io/warp-driver/warpdrive-stellar-middleware:latest

docker exec wdm warpdrive-deployer deploy --output-path /out/deploy.json
```

## CLI

| Subcommand | Args | Effect |
|---|---|---|
| `keygen` | `[--key-file <path>] [--friendbot-url <url>]` | Generates (if needed) and friendbot-funds a deployer identity, writing the secret to `--key-file` (default `/out/.keys/deployer.secret`). Idempotent. Prints the `G…` address. |
| `deploy` | `--output-path <file> [--variant <ethereum\|stellar>]` | Deploys a contract pipeline (security + verification) plus project-root and writes a JSON manifest to `<file>`. Default `ethereum` = secp256k1 pipeline; `stellar` = ed25519 pipeline. Idempotent: re-running resumes from the manifest. |
| `add-signer` | `--scheme {secp256k1,ed25519} --key <hex> --weight <u64> --deploy-file <path>` | Registers (or updates) a signer on the matching security contract. |
| `remove-signer` | `--scheme ... --key <hex> --deploy-file <path>` | Removes a signer. |
| `set-threshold` | `--scheme ... --numerator <u64> --denominator <u64> --deploy-file <path>` | Sets the consensus fraction. |
| `get-project-spec-repo` | `--deploy-file <path>` | Reads the project specification URL from the project-root contract. |
| `set-project-spec-repo` | `--repo <url> --deploy-file <path>` | Updates the project specification URL on the project-root contract (admin-only). |
| `get-ledger` | — | Prints the current ledger sequence (for `reference_block` lookups). |
| `help` | — | Prints usage (`warpdrive-deployer help` or `--help`). |

> **Parity break vs. the old shell CLI:** `--variant both` is **dropped**. Each
> manifest file is one pipeline; to provision both, run `deploy` twice into two
> files (`deploy.json` + `deploy-stellar.json`). Handler contracts are not
> deployed (unchanged from the shell deployer).

### Example

```bash
docker exec wdm warpdrive-deployer keygen

docker exec wdm warpdrive-deployer deploy --output-path /out/deploy.json

docker exec wdm warpdrive-deployer add-signer \
  --scheme secp256k1 --key 0xabcd... --weight 100 \
  --deploy-file /out/deploy.json

docker exec wdm warpdrive-deployer set-threshold \
  --scheme secp256k1 --numerator 2 --denominator 3 \
  --deploy-file /out/deploy.json

docker exec wdm warpdrive-deployer get-project-spec-repo \
  --deploy-file /out/deploy.json

docker exec wdm warpdrive-deployer set-project-spec-repo \
  --repo "ipfs://bafy.../spec.json" \
  --deploy-file /out/deploy.json
```

## Environment

Every value also has an equivalent flag (e.g. `--rpc-url`, `--secret`,
`--wasm-dir`); the flag wins over the env var.

| Var | Required by | Notes |
|---|---|---|
| `RPC_URL` | all subcommands that hit the network | e.g. `http://stellar:8000/rpc` (local Quickstart) or `https://soroban-testnet.stellar.org` (testnet) |
| `NETWORK_PASSPHRASE` | deploy / signers / project-root | e.g. `Standalone Network ; February 2017` (local Quickstart) or `Test SDF Network ; September 2015` (testnet) |
| `DEPLOYER_SECRET` | deploy / signers / project-root (BYOK) | Stellar secret seed (`S...`). The `G…` admin is derived from it. If unset, commands fall back to `--secret-file`, then the default keyfile written by `keygen`. |
| `KEY_FILE` | keygen (optional) | Keyfile path for `keygen`. Default `/out/.keys/deployer.secret`. |
| `FRIENDBOT_URL` | keygen (optional) | Friendbot endpoint used to fund the generated identity. For local Quickstart set e.g. `http://stellar:8000/friendbot`; leave unset for testnet/futurenet (derived via `getNetwork`). |
| `WASM_DIR` | deploy (optional) | Directory holding the contract wasm. Default `/warpdrive/wasm`. |
| `PROJECT_SPEC_REPO` | deploy (optional) | URL written into project-root at init. Default `ipfs://REPLACE_ME`. |
| `SECP_THRESHOLD_NUM` / `SECP_THRESHOLD_DEN` | deploy `--variant ethereum` (optional) | Initial threshold on secp256k1 security. Default `2/3`. |
| `ED_THRESHOLD_NUM` / `ED_THRESHOLD_DEN` | deploy `--variant stellar` (optional) | Initial threshold on ed25519 security. Default `2/3`. |
| `MAX_RETRIES` / `RETRY_SLEEP_SECONDS` | all invocations | Retry config for RPC hiccups. Default `3` / `5`. |

Removed vs. the shell image: `DEPLOYER_ADDRESS` (derived from the secret),
`INCLUSION_FEE` (fees come from simulation's `min_resource_fee`), and the
stellar-cli identity-store knobs `KEY_ALIAS` / `FUND_NETWORK`.

## Output manifest

`deploy` writes the IDs of the contracts it deployed for the chosen `--variant`.
Each file is a single pipeline; keys for the other variant are absent. The
schema is byte-compatible with the old shell deployer's output:

```json
{
  "admin": "G...",
  "rpc_url": "...",
  "network_passphrase": "...",
  "variant": "ethereum",
  "contracts": {
    "project_root": "C...",
    "secp256k1_security": "C...",
    "secp256k1_verification": "C..."
  }
}
```

A `stellar` deploy instead contains `ed25519_security` / `ed25519_verification`
plus `project_root`. The deploy is checkpointed after each contract, so a
mid-run abort + re-run resumes exactly where it stopped.

## Smoke testing

`smoke.sh` is a host-side wrapper that mounts `./out/` at `/out` and persists
the generated identity under `./out/.keys/` so the same admin is reused across
`docker run --rm` invocations. It runs `keygen` (idempotent) before any signing
command. Run it from the repository root.

By default it runs against testnet; pass `--network local` as the first
arg to spin up a local Stellar Quickstart sidecar (auto-started on docker
network `wdnet`). Use `./docker/middleware/smoke.sh down` to tear the
local sidecar back down — the host bind mount `./out/` is preserved.

### 1. Deploy

```bash
./docker/middleware/smoke.sh deploy --output-path /out/deploy.json
jq . out/deploy.json
```

First run on testnet generates + friendbot-funds the deployer identity and
deploys the default Ethereum (secp256k1) pipeline plus project-root.
Subsequent calls reuse the same identity.

To deploy the Stellar (ed25519) pipeline as well, use a second file:

```bash
./docker/middleware/smoke.sh deploy --variant stellar --output-path /out/deploy-stellar.json
```

For local Quickstart:

```bash
./docker/middleware/smoke.sh --network local deploy --output-path /out/deploy.json
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

./docker/middleware/smoke.sh set-threshold \
  --scheme secp256k1 --numerator 1 --denominator 2 \
  --deploy-file /out/deploy.json

./docker/middleware/smoke.sh remove-signer \
  --scheme secp256k1 --key "$SIGNER1_PUBKEY" \
  --deploy-file /out/deploy.json
```

Each call should print a transaction hash with no error.

### 4. Cross-check a deployed contract

Read the project spec repo back from project-root to confirm the deployment is
live and admin-readable:

```bash
./docker/middleware/smoke.sh get-project-spec-repo --deploy-file /out/deploy.json
```

Should print the `project_spec_repo` URL baked into project-root at deploy time.

### 5. Tear down

Testnet / BYOK (manual `docker run`):

```bash
docker rm -f wdm
```

Local Quickstart (compose):

```bash
docker compose -f docker/middleware/docker-compose.yml down
```

Local Quickstart (smoke.sh):

```bash
./docker/middleware/smoke.sh down
```

All variants leave `./out/` (including `./out/.keys/`) intact, so the next
`deploy` against testnet reuses the same friendbot-funded identity. For
local Quickstart the chain is wiped, so the contract IDs in
`out/deploy.json` from a prior session are invalid — re-run `deploy` to
refresh.
