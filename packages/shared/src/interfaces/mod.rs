use soroban_sdk::BytesN;

pub mod handler;
pub mod project_root;
pub mod security;
pub mod verification;
pub mod warpdrive;

/// Compressed secp256k1 public key (33 bytes).
/// This is a convenience alias for internal use — contract interface signatures
/// use `BytesN<33>` directly so the WASM spec resolves correctly.
pub type PubKey = BytesN<33>;
