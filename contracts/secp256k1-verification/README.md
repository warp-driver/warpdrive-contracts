# Verification Contract

The Verification contract performs EIP-191 secp256k1 signature verification for WarpDrive attestations. It is the cryptographic core of the verification pipeline -- it takes an envelope and a set of signatures, recovers the public key from each signature, verifies that each recovered key matches a registered signer in the Security contract, and checks that the cumulative weight of all valid signers meets the required threshold.

The contract is stateless beyond its configuration (admin address and linked Security contract). It does not store any attestation data; it purely validates and returns success or failure. This makes it reusable across different Handler implementations and composable with any contract that needs to verify Vectr attestations.

## Contract Interactions

**Security contract** -- The Verification contract calls the Security contract to look up signer weights and the required threshold. It uses historical weight queries (`get_signer_weights_at`, `required_weight_at`) when a `reference_block` is provided, ensuring verification uses the signer set that was active when the Vectrs signed.

**Handler contract** -- The Handler calls `verify` after decoding the envelope and extracting signatures. The Verification contract validates the signatures and returns success or a typed error. The Handler does not need to understand the cryptographic details.

**Off-chain components** -- Aggregators do not interact with this contract directly. They submit envelopes to the Handler, which delegates to Verification. However, off-chain tooling can call `check_one` to pre-validate a single signature before aggregation, or query `required_weight` and `signer_weight` for monitoring.

## Signature Verification Flow

1. Keccak256 hash the envelope bytes
2. EIP-191 wrap: `keccak256("\x19Ethereum Signed Message:\n32" || hash)`
3. `secp256k1_recover` from the digest + signature (65-byte r||s||v format, v=27/28)
4. Compress the recovered key and compare to the expected signer pubkey
5. Signatures must be provided in strict ascending order of signer pubkeys (prevents duplicates)
6. Cumulative weight of all valid signers must meet or exceed `required_weight`

## Interface

The full interface is defined in [`VerificationInterface`](https://github.com/warp-driver/warpdrive-contracts/blob/main/packages/shared/src/interfaces/verification.rs).

### State-Changing Actions

| Function | Description |
|----------|-------------|
| `upgrade(new_wasm_hash, new_version)` | Upgrade the contract WASM. Admin-only. |
| `propose_admin(new_admin)` | Propose a new admin (two-step transfer). Current admin only. |
| `accept_admin()` | Accept a pending admin transfer. Pending admin only. |

### Queries

| Function | Description |
|----------|-------------|
| `verify(envelope, signatures, signer_pubkeys, reference_block)` | Verify that the given signatures over the envelope carry sufficient weight. Signatures and pubkeys must be parallel arrays in strict ascending pubkey order. Returns `Ok(())` on success. |
| `check_one(envelope, signature, signer_pubkey, reference_block)` | Verify a single signature and return the signer's weight. Useful for pre-validation before aggregation. If `reference_block` is `None`, uses current weights. |
| `security_contract() -> Address` | Return the address of the linked Security contract. |
| `required_weight() -> u64` | Return the current required weight from the Security contract. |
| `signer_weight(signer_pubkey) -> u64` | Return the current weight of a signer from the Security contract. |
| `admin() -> Address` | Return the current admin address. |
| `pending_admin() -> Option<Address>` | Return the pending admin, if a transfer is in progress. |
| `version() -> String` | Return the current contract version. |

### Errors

| Code | Name | Description |
|------|------|-------------|
| 1 | `InvalidSignature` | Signature recovery did not produce the expected public key |
| 2 | `SignerNotRegistered` | Recovered signer has zero weight in the Security contract |
| 3 | `InsufficientWeight` | Cumulative signer weight is below the required threshold |
| 4 | `EmptySignatures` | No signatures were provided |
| 5 | `LengthMismatch` | Signatures and signer_pubkeys arrays have different lengths |
| 6 | `SignersNotOrdered` | Signer public keys are not in strict ascending order |

## Events

The Verification contract is stateless beyond configuration. Only upgrades emit events.

| Event | Topic | Data Fields | Emitted By |
|-------|-------|-------------|------------|
| `VerificationUpgraded` | -- | `version: String` | `upgrade` |

### VerificationUpgraded

Emitted when the contract WASM is upgraded.

| Field | Type | Topic | Description |
|-------|------|-------|-------------|
| `version` | `String` | no | New contract version |
