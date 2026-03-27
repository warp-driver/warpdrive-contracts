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

Install [Rust](https://rustup.rs/) (1.94.0+) and [Task](https://taskfile.dev/), then:

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
