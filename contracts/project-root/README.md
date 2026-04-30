# Project Root Contract

The Project Root contract is the root governance contract for a WarpDrive project. It serves as the on-chain anchor that the project's governance entity (a multisig, DAO, or single admin) controls. It records the addresses of the project's Security and Verification contracts, the URL of the project specification (typically an IPFS CID), and which cryptographic pipeline (`Ethereum` or `Stellar`) the project uses. Off-chain Vectrs and aggregators query the Project Root on startup to discover everything else they need.

This is a minimal admin contract -- it holds pointers, not logic. Its primary purpose is to provide a stable on-chain identity for the project and a single source of truth that governance can update without redeploying the rest of the stack.

## Stack Variants

The `verification_type` field is set once at construction time and tells callers which signature scheme the linked contracts use:

- **`Ethereum`** -- secp256k1 / EIP-191 / ABI-encoded envelopes. The linked Security and Verification contracts are the [secp256k1 variants](../secp256k1-security/) and the project's handler is an [Ethereum Handler](../ethereum-handler/). Use this when the same signed payloads need to be verifiable on both EVM chains and Stellar.
- **`Stellar`** -- ed25519 / SEP-0053 / XDR-encoded envelopes. The linked Security and Verification contracts are the [ed25519 variants](../ed25519-security/) and the project's handler is a [Stellar Handler](../stellar-handler/). Use this for Soroban-native projects that don't need EVM compatibility.

The Project Root itself is identical for both variants -- only the addresses it stores differ.

## Source Layout

| File | Purpose |
|------|---------|
| [`src/contract.rs`](./src/contract.rs) | Implements `ProjectRootInterface` and `WarpDriveInterface`; constructor wires admin + linked contracts + spec repo + verification type |
| [`src/storage.rs`](./src/storage.rs) | Persistent storage for admin, security/verification contract addresses, spec repo string, and verification type |
| [`src/lib.rs`](./src/lib.rs) | Crate root and module wiring |

## Contract Interactions

**Off-chain components** -- Vectrs and aggregators query the Project Root on startup to find:
1. The project specification (`project_spec_repo`) -- URL/CID of the spec containing circuit definitions, WASI binaries, and contract addresses.
2. The Security and Verification contract addresses (`security_contract`, `verification_contract`) for the active signer set.
3. The pipeline variant (`verification_type`) so they know which signature scheme to use.

When governance updates the spec repo via `update_project_spec_repo`, an `UpdatedSpecRepo` event is emitted; off-chain components subscribed to the event automatically pull the new specification.

**Project governance** -- The admin (governance entity) is the only address that can update the Project Root. The two-step admin transfer (`propose_admin` / `accept_admin`) ensures that governance transitions are explicit and require acceptance by the new admin. The [`warpdrive-client`](../../packages/client/) package provides a typed async client (`ProjectRoot`) for governance tooling.

**Other contracts** -- The Project Root does not call other contracts directly. The Security and Verification contract addresses are set at construction time and are exposed as queries; cross-contract relationships between Handler / Verification / Security are configured directly when those contracts are deployed.

## Interface

The full interface is defined in [`ProjectRootInterface`](../../packages/shared/src/interfaces/project_root.rs). Standard admin / upgrade / version methods come from [`WarpDriveInterface`](../../packages/shared/src/interfaces/warpdrive.rs).

### Constructor

| Parameter | Type | Description |
|-----------|------|-------------|
| `admin` | `Address` | Initial governance admin |
| `security_contract` | `Address` | Address of the project's Security contract (secp256k1 or ed25519) |
| `verification_contract` | `Address` | Address of the project's Verification contract (secp256k1 or ed25519) |
| `project_spec_repo` | `String` | URL/CID of the off-chain project specification |
| `verification_type` | `VerificationType` | `Ethereum` or `Stellar` -- identifies the pipeline variant |

### State-Changing Actions

| Function | Description |
|----------|-------------|
| `update_project_spec_repo(repo)` | Update the URL/CID of the project specification. Admin-only. Emits `UpdatedSpecRepo`. |
| `upgrade(new_wasm_hash, new_version)` | Upgrade the contract WASM. Admin-only. Emits `ContractUpgraded`. |
| `propose_admin(new_admin)` | Propose a new admin (two-step transfer). Current admin only. Emits `AdminProposed`. |
| `accept_admin()` | Accept a pending admin transfer. Pending admin only. Emits `AdminAccepted`. |

### Queries

| Function | Description |
|----------|-------------|
| `security_contract() -> Address` | Address of the linked Security contract. |
| `verification_contract() -> Address` | Address of the linked Verification contract. |
| `project_spec_repo() -> String` | Current URL/CID of the project specification. |
| `verification_type() -> VerificationType` | Which pipeline variant (`Ethereum` or `Stellar`) the linked contracts implement. |
| `admin() -> Address` | Current admin address. |
| `pending_admin() -> Option<Address>` | Pending admin, if a transfer is in progress. |
| `version() -> String` | Current contract version. |

### Types

Defined in [`packages/shared/src/interfaces/project_root.rs`](../../packages/shared/src/interfaces/project_root.rs):

- **`VerificationType`** -- enum with variants `Ethereum = 1` and `Stellar = 2`.

## Events

| Event | Topic | Data Fields | Emitted By |
|-------|-------|-------------|------------|
| `UpdatedSpecRepo` | -- | `repo: String` | `update_project_spec_repo` |
| `ContractUpgraded` | -- | `version: String` | `upgrade` |
| `AdminProposed` | -- | `old_admin: Address`, `new_admin: Address` | `propose_admin` |
| `AdminAccepted` | -- | `new_admin: Address` | `accept_admin` |
