# Handler Contract

This is a **reference implementation** of a WarpDrive handler -- a template demonstrating how to build the entry point for submitting cross-chain event envelopes to the WarpDrive verification pipeline. In this demo, the Handler simply stores the verified payload for later retrieval. A production handler would typically parse the payload into an application-specific format and use it to call other contracts (e.g., executing a swap on a DEX, updating an oracle price, minting tokens).

The Handler is designed to be the **admin or hold a privileged role** on those downstream contracts. Since only envelopes that pass full signature verification can trigger the Handler's actions, this effectively gates all downstream operations behind the consensus of the off-chain Vectrs running the project's defined WASI logic. This is the core pattern of WarpDrive: off-chain compute secured by on-chain verification, where the Handler is the bridge between the two.

When an aggregator submits an envelope, the Handler ABI-decodes it to extract the event ID and payload, enforces replay protection by tracking which event IDs have already been processed, and delegates cryptographic signature validation to the Verification contract. The Handler supports two envelope formats: `verify_eth` for Ethereum-originated ABI-encoded envelopes and `verify_xlm` for native Stellar-originated envelopes. Both paths share the same signature verification and replay protection logic.

## Contract Interactions

**Verification contract** -- The Handler calls the Verification contract via a lightweight client trait (not a crate import) to validate that the submitted signatures carry sufficient weight. The Verification contract in turn queries the Security contract for signer weights and thresholds. All verification errors propagate directly back to the Handler's caller.

**Off-chain components** -- Aggregators submit envelopes to the Handler after collecting sufficient Vectr attestations. The `reference_block` field in the signature data allows point-in-time weight lookups, ensuring that the signer set used for verification matches the set that was active when the Vectrs produced their attestations.

**Downstream contracts** -- In this reference implementation, verified payloads are simply stored and can be read via the `payload` query. In a production handler, this is where you would parse the payload and call application-specific contracts -- e.g., executing a trade, updating a price feed, or distributing rewards. The Handler would typically be the admin or hold a privileged role on those contracts, meaning the off-chain Vectr consensus effectively controls what actions are taken.

## Interface

The full interface is defined in [`HandlerInterface`](https://github.com/warp-driver/warpdrive-contracts/blob/main/packages/shared/src/interfaces/handler.rs).

### State-Changing Actions

| Function | Description |
|----------|-------------|
| `verify_eth(envelope_bytes, sig_data)` | ABI-decode an Ethereum-format envelope, verify signatures against the Verification contract, store the payload, and emit a `Verified` event. Fails if the event ID has already been seen. |
| `verify_xlm(envelope_bytes, sig_data)` | Verify a native Stellar envelope with the same signature and replay-protection flow as `verify_eth`. |
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

- **`SignatureData`** -- `{ signers: Vec<PubKey>, signatures: Vec<BytesN<65>>, reference_block: u32 }` -- bundled signature payload submitted by the aggregator.
- **`XlmEnvelope`** -- `{ event_id: BytesN<20>, ordering: BytesN<12>, payload: Bytes }` -- native Stellar envelope format.

### Errors

| Code | Name | Description |
|------|------|-------------|
| 1 | `EventAlreadySeen` | This event ID has already been verified and stored |
| 2 | `InvalidReferenceBlock` | The reference block is in the future |
| 3 | `InvalidEnvelope` | ABI decoding of the envelope failed |
| 20 | `UnknownVerificationError` | Unexpected error from the Verification contract |
| 21 | `InvalidSignature` | A signature did not recover to the expected public key |
| 22 | `SignerNotRegistered` | A signer is not in the Security contract's registry |
| 23 | `InsufficientWeight` | Total weight of valid signers is below the required threshold |
| 24 | `EmptySignatures` | No signatures were provided |
| 25 | `LengthMismatch` | Signers and signatures arrays have different lengths |
| 26 | `SignersNotOrdered` | Signer public keys are not in strict ascending order |

## Events

Events emitted by the Handler contract for off-chain indexing and monitoring.

| Event | Topic | Data Fields | Emitted By |
|-------|-------|-------------|------------|
| `Verified` | `event_id: BytesN<20>` | -- | `verify_eth`, `verify_xlm` |
| `HandlerUpgraded` | -- | `version: String` | `upgrade` |

### Verified

Emitted when a cross-chain event is successfully verified and stored.

| Field | Type | Topic | Description |
|-------|------|-------|-------------|
| `event_id` | `BytesN<20>` | yes | Unique cross-chain event identifier |

### HandlerUpgraded

Emitted when the contract WASM is upgraded.

| Field | Type | Topic | Description |
|-------|------|-------|-------------|
| `version` | `String` | no | New contract version |
