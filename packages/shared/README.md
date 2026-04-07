# Shared Package (`warpdrive-shared`)

Shared Rust library used by all WarpDrive contracts. Provides contract interface traits, reusable admin transfer logic, checkpoint-based historical storage, and test utilities.

## Modules

### [Interfaces](./src/interfaces/)

`#[contractclient]` trait definitions for all four contracts: [`HandlerInterface`](./src/interfaces/handler.rs), [`SecurityInterface`](./src/interfaces/security.rs), [`VerificationInterface`](./src/interfaces/verification.rs), and [`ProjectRootInterface`](./src/interfaces/project_root.rs). These traits generate lightweight client structs that contracts use for cross-contract calls without importing the actual contract crates. Also defines shared types (`PubKey`, `SignatureData`, `SignerInfo`), error enums (`HandlerError`, `SecurityError`, `VerifyError`), and the `XlmEnvelope` struct.

### [Admin](./src/admin.rs)

Two-step admin transfer module used by all contracts. The current admin calls `propose` to nominate a new admin, and the nominee calls `accept` to complete the transfer. Emits `AdminProposed` and `AdminAccepted` events.

### [Checkpoint](./src/checkpoint.rs)

Point-in-time snapshot storage backed by a `CheckpointStore` trait. Used by the Security contract to record signer weight changes at each ledger, enabling historical lookups via binary search. Supports same-ledger coalescing (multiple updates in one ledger are collapsed into a single checkpoint).

### [Test Utilities](./src/testutils.rs)

Feature-gated (`testutils`) helpers for generating deterministic secp256k1 signing keys, deriving compressed public keys, and producing Ethereum-style 65-byte signatures. Used across all contract test suites.
