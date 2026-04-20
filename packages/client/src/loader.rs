use soroban_rs::ContractId;
use std::fmt;
use std::fs;
use std::path::Path;

pub const TESTNET_DIR: &'static str = "../../.testnet";

#[derive(Clone, Debug)]
pub struct DeployConfig {
    pub ed25519_security: ContractId,
    pub ed25519_verification: ContractId,
    pub secp256k1_security: ContractId,
    pub secp256k1_verification: ContractId,
    pub ethereum_handler: ContractId,
    pub stellar_handler: ContractId,
    pub project_root: ContractId,
}

/// This loads all addresses from a testnet deploy.
/// It does not include the deployers secret key, which must be passed in separately.
pub fn testnet() -> DeployConfig {
    DeployConfig {
        ed25519_security: load_file(TESTNET_DIR, "ed-security.id"),
        ed25519_verification: load_file(TESTNET_DIR, "ed-verification.id"),
        secp256k1_security: load_file(TESTNET_DIR, "security.id"),
        secp256k1_verification: load_file(TESTNET_DIR, "verification.id"),
        ethereum_handler: load_file(TESTNET_DIR, "eth-handler.id"),
        stellar_handler: load_file(TESTNET_DIR, "xlm-handler.id"),
        project_root: load_file(TESTNET_DIR, "project-root.id"),
    }
}

impl fmt::Display for DeployConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "project_root: {}", self.project_root)?;
        writeln!(f, "ethereum_handler: {}", self.ethereum_handler)?;
        writeln!(f, "secp256k1_security: {}", self.secp256k1_security)?;
        writeln!(f, "secp256k1_verification: {}", self.secp256k1_verification)?;
        writeln!(f, "stellar_handler: {}", self.stellar_handler)?;
        writeln!(f, "ed25519_security: {}", self.ed25519_security)?;
        writeln!(f, "ed25519_verification: {}", self.ed25519_verification)
    }
}

fn load_file(dirname: &str, filename: &str) -> ContractId {
    let path = Path::new(dirname).join(filename);
    let contents = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read {}: {}", path.display(), e));
    let trimmed = contents.trim();
    ContractId::from_string(trimmed)
        .unwrap_or_else(|e| panic!("invalid contract id in {}: {:?}", path.display(), e))
}
