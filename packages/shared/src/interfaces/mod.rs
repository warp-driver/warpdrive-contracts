use soroban_sdk::BytesN;

pub mod handler;
pub mod project_root;
pub mod security;
pub mod verification;
pub mod warpdrive;

/// Compressed secp256k1 public key (33 bytes)
pub type PubKey = BytesN<33>;
