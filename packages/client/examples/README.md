# Client Examples

These show how to use the client and ensure it works properly before integrating in other codebases.

They both assume a tstnet deploy beforehand via: `task testnet:deploy && task testnet:setup-signers`

## Query

```bash
cargo run --example query
```

Does some basic sanity-check queries on the contract, doesn't require any keys.
You can override the default RPC handler via `XLM_RPC_URL`, but it is hardcoded to use the Testnet Passphrase.

## Execute

```bash
export XLM_SECRET_KEY=$(stellar keys secret warpdrive-test)
cargo run --example execute
```