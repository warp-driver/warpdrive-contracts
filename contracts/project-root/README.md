# Project Root Contract

The Project Root contract is the root governance contract for a WarpDrive project. It serves as the on-chain anchor that the project's governance entity (a multisig, DAO, or single admin) controls. In the broader WarpDrive architecture, the Project Root stores a pointer to the project's specification repository (on IPFS or similar content-addressable storage), which contains the circuit definitions, WASI binaries, and contract addresses that Vectrs need to operate.

This is a minimal admin contract. Its primary purpose is to provide a stable on-chain identity for the project that governance can update, and to serve as the entry point that Vectrs query when they start up or detect specification changes.

## Contract Interactions

**Off-chain components** -- Vectrs query the Project Root on startup to find the current project specification. They also subscribe to updates, so when governance changes the specification pointer, Vectrs automatically pull the new circuit definitions and reconfigure.

**Project governance** -- The admin (governance entity) is the only address that can update the Project Root. The two-step admin transfer ensures that governance transitions are explicit and require acceptance by the new admin.

**Other contracts** -- The Project Root does not call other contracts directly. It is referenced by the Verification and Security contracts as part of the project's deployment, but the relationships are configured at deployment time rather than enforced by cross-contract calls.

## Interface

The full interface is defined in [`ProjectRootInterface`](https://github.com/warp-driver/warpdrive-contracts/blob/main/packages/shared/src/interfaces/project_root.rs).

### State-Changing Actions

| Function | Description |
|----------|-------------|
| `upgrade(new_wasm_hash, new_version)` | Upgrade the contract WASM. Admin-only. |
| `propose_admin(new_admin)` | Propose a new admin (two-step transfer). Current admin only. |
| `accept_admin()` | Accept a pending admin transfer. Pending admin only. |

### Queries

| Function | Description |
|----------|-------------|
| `admin() -> Address` | Return the current admin address. |
| `pending_admin() -> Option<Address>` | Return the pending admin, if a transfer is in progress. |
| `version() -> String` | Return the current contract version. |

## Events

Minimal admin contract. Only upgrades emit events.

| Event | Topic | Data Fields | Emitted By |
|-------|-------|-------------|------------|
| `ProjectRootUpgraded` | -- | `version: String` | `upgrade` |

### ProjectRootUpgraded

Emitted when the contract WASM is upgraded.

| Field | Type | Topic | Description |
|-------|------|-------|-------------|
| `version` | `String` | no | New contract version |
