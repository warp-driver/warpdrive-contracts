# Project Root Contract Events

Minimal admin contract. Only upgrades emit events.

## Events

| Event | Topic | Data Fields | Emitted By |
|-------|-------|-------------|------------|
| `ProjectRootUpgraded` | — | `version: String` | `upgrade` |

### ProjectRootUpgraded

Emitted when the contract WASM is upgraded.

| Field | Type | Topic | Description |
|-------|------|-------|-------------|
| `version` | `String` | no | New contract version |
