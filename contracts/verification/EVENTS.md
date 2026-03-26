# Verification Contract Events

The verification contract is stateless beyond configuration. Only upgrades emit events.

## Events

| Event | Topic | Data Fields | Emitted By |
|-------|-------|-------------|------------|
| `VerificationUpgraded` | — | `version: String` | `upgrade` |

### VerificationUpgraded

Emitted when the contract WASM is upgraded.

| Field | Type | Topic | Description |
|-------|------|-------|-------------|
| `version` | `String` | no | New contract version |
