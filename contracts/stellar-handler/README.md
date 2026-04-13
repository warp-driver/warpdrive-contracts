# Stellar Handler Contract

This is a **reference implementation** of a WarpDrive handler for Soroban-native solutions. It uses Soroban-native XDR encoding, ed25519 cryptography, and Soroban precompiles for maximum efficiency and the best developer experience when building solutions that operate entirely within the Stellar ecosystem. It has no Solidity or alloy dependencies.

In this demo, the Handler simply stores the verified payload for later retrieval. A production handler would typically parse the payload into an application-specific format and use it to call other contracts (e.g., executing a swap on a DEX, updating an oracle price, minting tokens).

The Handler is designed to be the **admin or hold a privileged role** on those downstream contracts. Since only envelopes that pass full signature verification can trigger the Handler's actions, this effectively gates all downstream operations behind the consensus of the off-chain Vectrs running the project's defined WASI logic. This is the core pattern of WarpDrive: off-chain compute secured by on-chain verification, where the Handler is the bridge between the two.

When an aggregator submits an envelope, the Handler XDR-decodes it (Soroban native format) to extract the event ID and payload, enforces replay protection by tracking which event IDs have already been processed, and delegates ed25519 signature validation (SEP-0053 format) to the Ed25519 Verification contract.

## When to use this contract

Use the Stellar Handler when your WarpDrive process operates **natively on Soroban** and does not need to share signed payloads with EVM chains. Envelopes are XDR-encoded using Soroban types (`XlmEnvelope { event_id, ordering, payload }`), and signatures use ed25519 public keys with SEP-0053 message formatting. This gives you smaller payloads, cheaper verification (native Soroban precompiles), and a simpler dependency tree compared to the Ethereum-compatible variant.

For multi-chain processes where the same signed payload needs to be verifiable on both Ethereum and Stellar, see the [Ethereum Handler contract](../ethereum-handler/README.md).

## Contract Interactions

**Ed25519 Verification contract** -- The Handler calls the Verification contract via a lightweight client trait to validate that the submitted ed25519 signatures carry sufficient weight. The Verification contract uses Soroban's native `ed25519_verify` and `sha256` precompiles for verification, and queries the Ed25519 Security contract for signer weights and thresholds. All verification errors propagate directly back to the Handler's caller.

**Off-chain components** -- Aggregators submit envelopes to the Handler after collecting sufficient Vectr attestations. The `reference_block` field in the signature data allows point-in-time weight lookups, ensuring that the signer set used for verification matches the set that was active when the Vectrs produced their attestations.

**Downstream contracts** -- In this reference implementation, verified payloads are simply stored and can be read via the `payload` query. In a production handler, this is where you would parse the payload and call application-specific contracts.

## Interface

The full interface is defined in [`StellarHandlerInterface`](https://github.com/warp-driver/warpdrive-contracts/blob/main/packages/shared/src/interfaces/handler.rs).

### State-Changing Actions

| Function | Description |
|----------|-------------|
| `verify_xlm(envelope_bytes, sig_data)` | XDR-decode a Soroban-native envelope, verify ed25519 signatures against the Verification contract, store the payload, and emit a `Verified` event. Fails if the event ID has already been seen. |
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

- **`Ed25519SignatureData`** -- `{ signers: Vec<Ed25519PubKey>, signatures: Vec<BytesN<64>>, reference_block: u32 }` -- bundled ed25519 signature payload submitted by the aggregator.
- **`XlmEnvelope`** -- `{ event_id: BytesN<20>, ordering: BytesN<12>, payload: Bytes }` -- native Stellar envelope format.

### Errors

| Code | Name | Description |
|------|------|-------------|
| 501 | `EventAlreadySeen` | This event ID has already been verified and stored |
| 502 | `InvalidReferenceBlock` | The reference block is in the future or too old |
| 503 | `InvalidEnvelope` | XDR decoding of the envelope failed |
| 504 | `UnknownVerificationError` | Unexpected error from the Verification contract |
| 505 | `OtherInvocationError` | Cross-contract invocation failed |
| 301 | `InvalidSignature` | Signature verification failed (all-zero sigs only; non-zero invalid sigs cause a host error) |
| 302 | `SignerNotRegistered` | A signer is not in the Security contract's registry |
| 303 | `InsufficientWeight` | Total weight of valid signers is below the required threshold |
| 304 | `EmptySignatures` | No signatures were provided |
| 305 | `LengthMismatch` | Signers and signatures arrays have different lengths |
| 306 | `SignersNotOrdered` | Signer public keys are not in strict ascending order |
| 307 | `ZeroRequiredWeight` | Required weight is zero |

## Events

| Event | Topic | Data Fields | Emitted By |
|-------|-------|-------------|------------|
| `Verified` | `event_id: BytesN<20>` | -- | `verify_xlm` |
| `ContractUpgraded` | -- | `version: String` | `upgrade` |
