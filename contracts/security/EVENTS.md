# Security Contract Events

All state-changing operations emit events for off-chain indexing and monitoring.

## Events

| Event | Topic | Data Fields | Emitted By |
|-------|-------|-------------|------------|
| `SignerAdded` | `key: BytesN<33>` | `weight: u64` | `add_signer` |
| `SignerRemoved` | `key: BytesN<33>` | — | `remove_signer` |
| `ThresholdSet` | — | `numerator: u64`, `denominator: u64` | `set_threshold` |
| `Upgraded` | — | `version: String` | `upgrade` |

### SignerAdded

Emitted when a signer is added or their weight is updated.

| Field | Type | Topic | Description |
|-------|------|-------|-------------|
| `key` | `BytesN<33>` | yes | Compressed secp256k1 public key |
| `weight` | `u64` | no | New signer weight |

### SignerRemoved

Emitted when a signer is removed.

| Field | Type | Topic | Description |
|-------|------|-------|-------------|
| `key` | `BytesN<33>` | yes | Compressed secp256k1 public key |

### ThresholdSet

Emitted when the verification threshold is changed.

| Field | Type | Topic | Description |
|-------|------|-------|-------------|
| `numerator` | `u64` | no | Threshold numerator |
| `denominator` | `u64` | no | Threshold denominator |

### Upgraded

Emitted when the contract WASM is upgraded.

| Field | Type | Topic | Description |
|-------|------|-------|-------------|
| `version` | `String` | no | New contract version |
