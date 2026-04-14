# WarpDrive Contracts

Soroban smart contracts for [WarpDrive](https://warp-drive.xyz), a platform for enterprise-grade, verifiable off-chain compute on the Stellar network. This repository is the deliverable for **Milestone 2: Soroban Security Contracts (PoA)** of [the WarpDrive proposal](https://ipfs.io/ipfs/bafybeifl56dmzfy6svrbb3in724bmrmh3bltah77t7b56vq3znhlb7w3ba).

## Overview

WarpDrive provides a trusted compute layer that turns arbitrary off-chain data and processes into provably correct on-chain actions. Off-chain execution nodes called **Vectrs** run user-defined circuits, produce signed attestations, and submit them to on-chain contracts for verification. These contracts form the on-chain trust layer that validates Vectr attestations before any state changes are committed to the Stellar ledger.

The contracts in this repository implement the core verification pipeline:

```
Handler --> Verification --> Security
```

- **Security** maintains a Proof-of-Authority registry of trusted Vectr public keys and their weights, and computes the threshold required for valid attestation.
- **Verification** performs secp256k1 signature recovery (EIP-191) and checks that submitted signatures carry enough cumulative weight against the Security contract's threshold.
- **Handler** is the entry point for cross-chain envelopes. It ABI-decodes the payload, enforces replay protection, and delegates cryptographic validation to the Verification contract.
- **Project Root** is the root governance contract for a WarpDrive project, controlled by the project's admin.

## Quick Start

Install [Rust](https://rustup.rs/) (1.94.0+) and [Task](https://taskfile.dev/), then install the `wasm32v1-none` target and the [Stellar CLI](https://developers.stellar.org/docs/tools/developer-tools/cli/install-cli) (required for `task optimize` and `task deploy`):

```bash
task setup          # rustup target add wasm32v1-none + cargo install stellar-cli --locked
```

On Ubuntu/Debian, `stellar-cli` needs a few system libraries first:

```bash
sudo apt install -y build-essential pkg-config libdbus-1-dev libudev-dev
```

For other platforms, see the [official install guide](https://developers.stellar.org/docs/tools/developer-tools/cli/install-cli).

Then:

```bash
task build          # Build all contracts to WASM
task test           # Run all unit tests (builds first)
task check          # Quick cargo check without WASM build
task fmt            # Format code
task clippy         # Lint (warnings are errors)
task lint           # fmt-check + clippy
task optimize       # Stellar contract optimization for deployment
```

Run a single contract's tests:

```bash
cargo test -p warpdrive-handler
cargo test -p warpdrive-security
cargo test -p warpdrive-verification
cargo test -p warpdrive-project-root
```

Run a single test:

```bash
cargo test -p warpdrive-handler test_verify_success
```

## Deployment

You can build and optimize all contracts and then deploy them to testnet, with one command:

```bash
task testnet:deploy
```

You can then run some manual tests like:

```bash
task testnet:setup-signers

# should pass first time (second time is error 501 - EventAlreadySeen)
task testnet:eth-test-happy
# Should fail with 303 (InsufficientWeight)
task testnet:eth-test-insufficient
# Should fail with 301 (InvalidSignature)
task testnet:eth-test-invalid-sig

# Should pass first time (second time is error 501 - EventAlreadySeen)
task testnet:xlm-test-happy
# Should fail with 303 (InsufficientWeight)
task testnet:xlm-test-insufficient
# Should fail with 505 (OtherInvocationError) - panic on invalid ed25519 signature
task testnet:xlm-test-invalid-sig
```

Note: In order for this to work, you must have previously configured stellar-cli: `task setup`

### IPFS Project Specification

After deploying contracts, you can publish the project specification to IPFS via [Pinata](https://app.pinata.cloud). This pins a `spec.json` containing contract IDs, WASM hashes, and deployment metadata - the file that Vectrs query on startup.

```bash
# Set your Pinata JWT (get one at https://app.pinata.cloud/developers/api-keys)
export PINATA_JWT=<your-jwt>

# Build spec.json from deployment state, upload to Pinata, update on-chain
task ipfs:publish
```

Or run each step individually:

```bash
task ipfs:build-spec       # Assemble spec.json + copy WASMs from deploy state
task ipfs:pin              # Upload spec.json to Pinata, get CID
task ipfs:update-contract  # Set project_spec_repo to ipfs://<CID> on-chain
task ipfs:status           # Show current CID and on-chain value
```

## Contracts and Packages

### Contracts

| Contract | Description |
|----------|-------------|
| [Handler](./contracts/handler/) | Entry point for cross-chain envelopes; ABI-decodes payloads, enforces replay protection, and delegates signature verification |
| [Security](./contracts/security/) | Proof-of-Authority signer registry with weighted keys and configurable verification thresholds |
| [Verification](./contracts/verification/) | EIP-191 secp256k1 signature verification against the Security contract's signer set |
| [Project Root](./contracts/project-root/) | Minimal root governance contract for a WarpDrive project |

### Packages

| Package | Description |
|---------|-------------|
| [Shared](./packages/shared/) | Shared library providing contract interfaces, admin transfer logic, checkpoint storage, and test utilities |

## License

GPL-3.0-or-later
