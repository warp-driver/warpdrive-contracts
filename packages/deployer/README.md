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
deploy                  # deploy a pipeline + project-root; project_root adopts it
deploy-handler          # deploy the handler (admin=project_root) + auto-register
register-handler        # register a handler with project_root (admin write)
list-handlers           # list the handlers project_root governs
add-signer              # register/update a signer (--scheme secp256k1|ed25519)
remove-signer           # drop a signer
set-threshold           # set numerator/denominator
get-project-spec-repo   # read project_spec_repo
set-project-spec-repo   # update project_spec_repo (admin)
get-ledger              # print the latest ledger sequence
propose-admin           # start rotating a contract's admin (--target ...)
accept-admin            # accept admin, signed by the pending admin
accept-contract-admin   # project_root accepts a downstream's admin
handover                # rotate project_root's own admin to the owner (step 5)
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
old shell deployer's `deploy.json` for handler-free deployments — the optional
`ethereum_handler` / `stellar_handler` slots are only written once you run
`deploy-handler`, so a `deploy`-only manifest is unchanged.

## Ownership

`deploy` does more than deploy: after creating security, verification and
project_root, it **adopts** the downstreams — rotating their admin to
project_root — so the whole pipeline ends up owned by project_root, with the
deployer left as project_root's admin. (This is "step 4" of the deployment; the
adoption is idempotent, so re-running `deploy` is safe.) Consequently, signer
and threshold changes go *through* project_root (`--via-project-root`) even
before any handover — the deployer is no longer the security contract's admin.
`handover` is then just the final step: rotating project_root's own admin to the
owner.

## Handlers

Handler contracts are deployed and tracked separately from the pipeline:

```bash
# deploy the variant's handler (admin = project_root); auto-registers if the
# deployer is still project_root's admin
warpdrive-deployer deploy-handler --deploy-file /out/deploy.json
# confirm it's tracked
warpdrive-deployer list-handlers --deploy-file /out/deploy.json
```

`deploy-handler` deploys the variant's handler with **project_root as its admin**
(pointing at the manifest's verification contract), records it in the manifest,
and then tracks it in project_root's handler set — but only if this deployer is
project_root's admin (it queries the current admin). Post-handover, project_root
is owned by someone else, so `deploy-handler` instead prints a note that the
project_root admin must run `register-handler`:

```bash
# run by the project_root admin (e.g. the owner, after handover)
warpdrive-deployer register-handler --deploy-file /out/deploy.json --secret S<owner>
```

`register-handler` calls project_root's `register_handler` (an admin write) — a
single call, no admin-handover dance. Both `deploy-handler` and
`register-handler` are idempotent.

**Alternative (handover) flow.** A handler deployed under a different admin can
instead be brought in by handing its admin to project_root —
`propose-admin --target handler` then `accept-contract-admin --target handler` —
which auto-registers it on accept. Either way, removing a handler from the set
is always explicit (`unregister-handler`); rotating its admin away does not
untrack it.

## Docker

This binary is packaged into `ghcr.io/warp-driver/warpdrive-stellar-middleware`.
See [`docker/middleware/README.md`](../../docker/middleware/README.md) for the
`docker run` / `docker exec warpdrive-deployer …` invocation and the `smoke.sh`
wrapper.

## Governance handover

The downstreams are already owned by project_root (adopted during `deploy` —
see [Ownership](#ownership)), so `handover --owner <G…>` only does the final
step: proposing project_root's own admin to the owner. It's idempotent (guards
on `admin()`/`pending_admin()` reads), so a re-run resumes. The owner finishes
with their own key:

```bash
warpdrive-deployer accept-admin --target project-root --secret S<owner> --deploy-file /out/deploy.json
```

After handover, signer changes route through project_root:
`add-signer --via-project-root …`. The behaviour is exercised end-to-end by the
`#[ignore]`d `tests/governance.rs` against a local Quickstart.
