# Handler Contract Events

Events emitted by the handler contract for off-chain indexing and monitoring.

## Events

| Event | Topic | Data Fields | Emitted By |
|-------|-------|-------------|------------|
| `Verified` | `event_id: BytesN<20>` | — | `verify_eth`, `verify_xlm` |
| `HandlerUpgraded` | — | `version: String` | `upgrade` |

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
