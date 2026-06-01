# warpdrive-deployer

Native Rust CLI that deploys and manages the WarpDrive Stellar contracts,
driving [`warpdrive-client`](../client) / `wasi-soroban-rs` directly. Replaces
the old `docker/middleware/*.sh` shell + `stellar` CLI layer (GitHub issue #49).

It's a standalone crate, **excluded from the contract workspace** (like
`packages/client`) so its `clap`/`tokio`/`serde`/`reqwest` deps never reach the
`wasm32v1-none` contract builds. Build it from this directory:

```bash
cargo build --release          # -> target/release/warpdrive-deployer
cargo test                     # pure + mocked unit/integration tests
cargo test -- --ignored        # opt-in end-to-end against a local Quickstart
```

## Design

Every subcommand is a thin wrapper in `main.rs` over a typed function in the
library (`deploy`, `signers`, `project_root`, `ledger`, `identity`). `main.rs`
is the only place that reads argv/env and writes stdout; the typed functions are
unit-testable directly.

| Module | Responsibility |
|---|---|
| `cli` | clap derive: `Cli`, `Command`, per-command arg structs |
| `config` | `NetworkConfig` → `Env`; wasm-dir + client-config resolution |
| `identity` | BYOK secret resolution, keyfile I/O, `keygen` generate+fund |
| `manifest` | re-export of the shared `StellarDeployManifest` (in `warpdrive-client`) |
| `deploy` | idempotent deploy pipeline + constructor-arg encoding |
| `signers` | `add`/`remove`-signer, `set-threshold` (direct) + key validation |
| `project_root` | `get`/`set`-project-spec-repo |
| `ledger` | `get-latest-ledger` |
| `retry` | generic async retry (`MAX_RETRIES` / `RETRY_SLEEP_SECONDS`) |
| `error` | `DeployerError` (thiserror) |

## Subcommands

```text
keygen                  # generate + friendbot-fund an identity keyfile
deploy                  # deploy a pipeline (ethereum | stellar) + project-root
add-signer              # register/update a signer (--scheme secp256k1|ed25519)
remove-signer           # drop a signer
set-threshold           # set numerator/denominator
get-project-spec-repo   # read project_spec_repo
set-project-spec-repo   # update project_spec_repo (admin)
get-ledger              # print the latest ledger sequence
propose-admin           # start rotating a contract's admin (--target ...)
accept-admin            # accept admin, signed by the pending admin
accept-contract-admin   # project_root accepts a downstream's admin
handover                # composite: downstreams -> project_root -> owner
help                    # usage
```

`add-signer` / `remove-signer` / `set-threshold` accept `--via-project-root` to
route through project_root's forwarder — the post-handover mode in which the
deployer/owner can no longer call the security contract directly.

Run `warpdrive-deployer <subcommand> --help` for the full flag list. Identity
resolution precedence for the signing commands: `--secret` → `--secret-file` →
`DEPLOYER_SECRET` → default keyfile (`/out/.keys/deployer.secret`). The `G…`
admin address is derived from the secret.

The deploy manifest is one pipeline per file (no `--variant both`); run `deploy`
twice into two files to provision both. The schema is byte-compatible with the
old shell deployer's `deploy.json`. Handler contracts are not deployed (docker
parity).

## Docker

This binary is packaged into `ghcr.io/warp-driver/warpdrive-stellar-middleware`.
See [`docker/middleware/README.md`](../../docker/middleware/README.md) for the
`docker run` / `docker exec warpdrive-deployer …` invocation and the `smoke.sh`
wrapper.

## Governance handover

`handover --owner <G…>` runs the full rotation (PLAN.md §5): each downstream's
admin is proposed to project_root and accepted via `accept-contract-admin`, then
project_root's own admin is proposed to the owner. It's idempotent (guards on
`admin()`/`pending_admin()` reads), so a re-run resumes. The owner finishes with
their own key:

```bash
warpdrive-deployer accept-admin --target project-root --secret S<owner> --deploy-file /out/deploy.json
```

After handover, signer changes route through project_root:
`add-signer --via-project-root …`. The behaviour is exercised end-to-end by the
`#[ignore]`d `tests/governance.rs` against a local Quickstart.
