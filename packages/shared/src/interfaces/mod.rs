use soroban_sdk::BytesN;

pub mod handler;
pub mod project_root;
pub mod security;
pub mod verification;
pub mod warpdrive;

/// Compressed secp256k1 public key
/// Can generate this with compressed_pubkey in testutils.rs
pub type CompressedSecpPubKey = BytesN<33>;

/// Secp256k1 signature with recovery byte
pub type SecpSignature = BytesN<65>;

/// ed25519 public key
pub type Ed25519PubKey = BytesN<32>;

/// ed25519 public key
pub type Ed25519Signature = BytesN<64>;
