# Shared Package (`warpdrive-shared`)

Shared `no_std` Rust library used by all WarpDrive contracts. Provides contract interface traits, reusable admin transfer logic, checkpoint-based historical storage, TTL constants, and test utilities.

## Modules

### [Interfaces](./src/interfaces/)

`#[contractclient]` trait definitions for every WarpDrive contract. Each trait generates a lightweight client struct (e.g. `EthereumHandlerClient`) that contracts use for cross-contract calls without importing the actual contract crates. Defining the interface in this shared package -- rather than in each contract crate -- guarantees that on-chain and off-chain callers see the same shape, and lets the [`warpdrive-client`](../client/) package generate its async wrappers from a single source.

| Module | Trait(s) | Used by |
|--------|----------|---------|
| [`warpdrive.rs`](./src/interfaces/warpdrive.rs) | `WarpDriveInterface` -- standard `upgrade`, `propose_admin`, `accept_admin`, `admin`, `pending_admin`, `version` for every contract | All contracts |
| [`handler.rs`](./src/interfaces/handler.rs) | `EthereumHandlerInterface`, `StellarHandlerInterface` (also defines `SignatureData`, `Ed25519SignatureData`, `XlmEnvelope`, `Verified` event, `HandlerError` 501-505) | [`ethereum-handler`](../../contracts/ethereum-handler/), [`stellar-handler`](../../contracts/stellar-handler/) |
| [`security.rs`](./src/interfaces/security.rs) | `Secp256k1SecurityInterface`, `Ed25519SecurityInterface` (also defines `SignerInfo`, `Ed25519SignerInfo`, signer-add/remove + threshold events, `SecurityError` 201-204) | [`secp256k1-security`](../../contracts/secp256k1-security/), [`ed25519-security`](../../contracts/ed25519-security/) |
| [`verification.rs`](./src/interfaces/verification.rs) | `Secp256k1VerificationInterface`, `Ed25519VerificationInterface` (also defines `VerifyError` 301-307) | [`secp256k1-verification`](../../contracts/secp256k1-verification/), [`ed25519-verification`](../../contracts/ed25519-verification/) |
| [`project_root.rs`](./src/interfaces/project_root.rs) | `ProjectRootInterface`, the `VerificationType` enum (`Ethereum` / `Stellar`), and the `UpdatedSpecRepo` event | [`project-root`](../../contracts/project-root/) |
| [`mod.rs`](./src/interfaces/mod.rs) | Type aliases: `CompressedSecpPubKey = BytesN<33>`, `SecpSignature = BytesN<65>`, `Ed25519PubKey = BytesN<32>`, `Ed25519Signature = BytesN<64>` | All contracts |

### [Admin](./src/admin.rs)

Two-step admin transfer used by all contracts. The current admin calls `propose_admin` to nominate a new admin, and the nominee calls `accept_admin` to complete the transfer. Emits `AdminProposed` and `AdminAccepted` events (defined in [`interfaces/warpdrive.rs`](./src/interfaces/warpdrive.rs)).

### [Checkpoint](./src/checkpoint.rs)

Point-in-time snapshot storage backed by a `CheckpointStore` trait. Used by both Security contracts to record signer weight changes at each ledger, enabling historical lookups via binary search. Supports same-ledger coalescing (multiple updates in one ledger collapse into a single checkpoint). The Security contracts implement `CheckpointStore` over their own persistent storage; the binary search and pruning logic lives here.

### [Vec History](./src/vec_history.rs)

Generic pruned-history timeline (`VecHistoryStore` trait + `Entry<T>` type) that backs the `Checkpoint` module. Entries are kept sorted by ledger and pruned past a configurable cutoff window.

### [TTL](./src/ttl.rs)

Storage TTL constants used by every contract: instance/persistent target TTLs and renewal thresholds (in ledger units). Centralizing these keeps TTL behavior consistent across the stack.

### [Test Utilities](./src/testutils.rs)

Feature-gated (`testutils`) helpers used across all contract test suites:
- `make_secp256k1_key(seed)` / `secp256k1_pubkey(env, key)` / `secp256k1_sign_envelope(key, envelope)` -- deterministic secp256k1 signing keys and EIP-191 signatures (k256 + sha3).
- `make_ed25519_key(seed)` / `ed25519_pubkey(env, key)` / `ed25519_sign_envelope(key, envelope)` -- deterministic ed25519 keys and SEP-0053 signatures (ed25519-dalek).
