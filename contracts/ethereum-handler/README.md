# Ethereum Handler Contract

This is a **reference implementation** of a WarpDrive handler for multi-chain processes that need Ethereum compatibility. It uses the same Ethereum envelope format (ABI-encoded with Solidity types) that is used on EVM chains, so a single signed payload can be verified on both Ethereum and Stellar. This makes it the right choice for any WarpDrive deployment where the same Vectr attestations are submitted to contracts on multiple chains.

In this demo, the Handler simply stores the verified payload for later retrieval. A production handler would typically parse the payload into an application-specific format and use it to call other contracts (e.g., executing a swap on a DEX, updating an oracle price, minting tokens).

The Handler is designed to be the **admin or hold a privileged role** on those downstream contracts. Since only envelopes that pass full signature verification can trigger the Handler's actions, this effectively gates all downstream operations behind the consensus of the off-chain Vectrs running the project's defined WASI logic. This is the core pattern of WarpDrive: off-chain compute secured by on-chain verification, where the Handler is the bridge between the two.

When an aggregator submits an envelope, the Handler ABI-decodes it (Solidity-compatible format) to extract the event ID and payload, enforces replay protection by tracking which event IDs have already been processed, and delegates secp256k1 signature validation (EIP-191 format) to the Secp256k1 Verification contract.

## When to use this contract

Use the Ethereum Handler when your WarpDrive process involves **multi-chain communication** where the same signed payload needs to be verifiable on both Ethereum (or other EVM chains) and Stellar. Envelopes are ABI-encoded using Solidity types (`bytes20 eventId, bytes12 ordering, bytes payload`), and signatures use compressed secp256k1 public keys with EIP-191 formatting -- the same format used by Ethereum smart contracts.

For Soroban-only solutions that don't need EVM compatibility, see the [Stellar Handler contract](../stellar-handler/README.md), which uses native Soroban encoding and ed25519 cryptography for better efficiency and developer experience.

## Contract Interactions

**Secp256k1 Verification contract** -- The Handler calls the Verification contract via a lightweight client trait to validate that the submitted secp256k1 signatures carry sufficient weight. The Verification contract in turn queries the Secp256k1 Security contract for signer weights and thresholds. All verification errors propagate directly back to the Handler's caller.

**Off-chain components** -- Aggregators submit envelopes to the Handler after collecting sufficient Vectr attestations. The `reference_block` field in the signature data allows point-in-time weight lookups, ensuring that the signer set used for verification matches the set that was active when the Vectrs produced their attestations.

**Downstream contracts** -- In this reference implementation, verified payloads are simply stored and can be read via the `payload` query. In a production handler, this is where you would parse the payload and call application-specific contracts.

## Interface

The full interface is defined in [`EthereumHandlerInterface`](https://github.com/warp-driver/warpdrive-contracts/blob/main/packages/shared/src/interfaces/handler.rs).

### State-Changing Actions

| Function | Description |
|----------|-------------|
| `verify_eth(envelope_bytes, sig_data)` | ABI-decode an Ethereum-format envelope, verify secp256k1 signatures against the Verification contract, store the payload, and emit a `Verified` event. Fails if the event ID has already been seen. |
| `upgrade(new_wasm_hash, new_version)` | Upgrade the contract WASM. Admin-only. |
| `propose_admin(new_admin)` | Propose a new admin (two-step transfer). Current admin only. |
| `accept_admin()` | Accept a pending admin transfer. Pending admin only. |

### Queries

| Function | Description |
|----------|-------------|
| `payload(event_id) -> Option<Bytes>` | Return the stored payload for a verified event ID, or `None` if it hasn't been verified. |
| `verification_contract() -> Address` | Return the address of the linked Verification contract. |
| `admin() -> Address` | Return the current admin address. |
| `pending_admin() -> Option<Address>` | Return the pending admin, if a transfer is in progress. |
| `version() -> String` | Return the current contract version. |

### Types

- **`SignatureData`** -- `{ signers: Vec<CompressedSecpPubKey>, signatures: Vec<BytesN<65>>, reference_block: u32 }` -- bundled secp256k1 signature payload submitted by the aggregator.

### Errors

| Code | Name | Description |
|------|------|-------------|
| 501 | `EventAlreadySeen` | This event ID has already been verified and stored |
| 502 | `InvalidReferenceBlock` | The reference block is in the future or too old |
| 503 | `InvalidEnvelope` | ABI decoding of the envelope failed |
| 504 | `UnknownVerificationError` | Unexpected error from the Verification contract |
| 505 | `OtherInvocationError` | Cross-contract invocation failed |
| 301 | `InvalidSignature` | A signature did not recover to the expected public key |
| 302 | `SignerNotRegistered` | A signer is not in the Security contract's registry |
| 303 | `InsufficientWeight` | Total weight of valid signers is below the required threshold |
| 304 | `EmptySignatures` | No signatures were provided |
| 305 | `LengthMismatch` | Signers and signatures arrays have different lengths |
| 306 | `SignersNotOrdered` | Signer public keys are not in strict ascending order |
| 307 | `ZeroRequiredWeight` | Required weight is zero |

## Events

| Event | Topic | Data Fields | Emitted By |
|-------|-------|-------------|------------|
| `Verified` | `event_id: BytesN<20>` | -- | `verify_eth` |
| `ContractUpgraded` | -- | `version: String` | `upgrade` |
