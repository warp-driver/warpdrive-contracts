use std::fmt;
use std::fs;
use std::path::Path;
use wasi_soroban_rs::ContractId;

pub const TESTNET_DIR: &str = "../../.testnet";

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
///
/// Reads the legacy `.testnet/*.id` files written by `taskfiles/testnet.yml`.
/// New code should prefer [`StellarDeployManifest`] (the `deploy.json` schema the
/// Rust deployer reads/writes); `.testnet/*.id` is on a deprecation path and will
/// be retired once the manifest grows handler slots.
#[deprecated(note = "use StellarDeployManifest; .testnet/*.id is being retired")]
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

// ── Shared deploy.json manifest (behind the `manifest` feature) ─────────────
//
// One file is one pipeline (`ethereum` *or* `stellar`); the schema is
// byte-compatible with the shell deployer's `deploy.sh` output so existing
// `jq -e` consumers keep working:
//
// ```json
// {
//   "admin": "G...",
//   "rpc_url": "...",
//   "network_passphrase": "...",
//   "variant": "ethereum",
//   "contracts": { "project_root": "C...", "secp256k1_security": "C...",
//                  "secp256k1_verification": "C..." }
// }
// ```
//
// The plan (PLAN.md §7) sketches a `variant`-tagged sum type via
// `#[serde(flatten)]` over an adjacently-tagged enum. We instead model
// `variant` as a sibling field and the contract IDs as `Option`s on a single
// struct. Two reasons:
//   1. Idempotency. `deploy` checkpoints after *each* contract, so partial
//      files (only `secp256k1_security` set, say) must round-trip. A
//      non-`Option` enum arm can't represent that intermediate state.
//   2. Robustness. `flatten` + adjacently-tagged enums are a known serde_json
//      sharp edge; siblings + `Option` avoid it entirely.
// The wire format is identical to the sketch, and the golden-file round-trip
// test (`tests/manifest.rs`) guards it.
#[cfg(feature = "manifest")]
mod manifest_impl {
    use super::ContractId;
    use crate::project_root::VerificationType;
    use serde::{Deserialize, Serialize};
    use std::io;
    use std::path::Path;

    /// Which signature pipeline a manifest file describes.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(rename_all = "lowercase")]
    pub enum Variant {
        Ethereum,
        Stellar,
    }

    impl Variant {
        /// The `VerificationType` project_root is constructed with for this
        /// pipeline by default.
        pub fn default_verification_type(self) -> VerificationType {
            match self {
                Variant::Ethereum => VerificationType::Ethereum,
                Variant::Stellar => VerificationType::Stellar,
            }
        }
    }

    impl std::fmt::Display for Variant {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Variant::Ethereum => f.write_str("ethereum"),
                Variant::Stellar => f.write_str("stellar"),
            }
        }
    }

    /// The set of deployed contract IDs. Empty slots are omitted on the wire so
    /// a partial (mid-deploy) file stays valid and `jq -e` on a present key
    /// either reads a `C…` or fails loudly.
    #[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
    pub struct ManifestContracts {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub project_root: Option<ContractId>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub secp256k1_security: Option<ContractId>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub secp256k1_verification: Option<ContractId>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub ed25519_security: Option<ContractId>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub ed25519_verification: Option<ContractId>,
    }

    /// A single deployed pipeline + project_root, read from / written to
    /// `deploy.json`. Shared between the deployer (writer) and client consumers
    /// that only need project_root plus the active pipeline (reader).
    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    pub struct StellarDeployManifest {
        pub admin: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub rpc_url: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub network_passphrase: Option<String>,
        pub variant: Variant,
        #[serde(default)]
        pub contracts: ManifestContracts,
    }

    impl StellarDeployManifest {
        /// A fresh, empty manifest for `variant` (no contracts deployed yet).
        pub fn new(admin: String, variant: Variant) -> Self {
            Self {
                admin,
                rpc_url: None,
                network_passphrase: None,
                variant,
                contracts: ManifestContracts::default(),
            }
        }

        /// The project_root contract ID, if deployed (variant-independent).
        pub fn project_root(&self) -> Option<ContractId> {
            self.contracts.project_root
        }

        /// The security contract ID for this manifest's variant, if deployed.
        pub fn security(&self) -> Option<ContractId> {
            match self.variant {
                Variant::Ethereum => self.contracts.secp256k1_security,
                Variant::Stellar => self.contracts.ed25519_security,
            }
        }

        /// The verification contract ID for this manifest's variant, if deployed.
        pub fn verification(&self) -> Option<ContractId> {
            match self.variant {
                Variant::Ethereum => self.contracts.secp256k1_verification,
                Variant::Stellar => self.contracts.ed25519_verification,
            }
        }

        /// Read a manifest from `path`. Returns an `InvalidData` io error if the
        /// file is not valid JSON for this schema.
        pub fn load(path: &Path) -> io::Result<Self> {
            let bytes = std::fs::read(path)?;
            serde_json::from_slice(&bytes)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
        }

        /// Read a manifest from `path` if it exists, else `None`. A present but
        /// malformed file is an error (so a typo doesn't silently start a fresh
        /// deploy on top of real contracts).
        pub fn load_if_exists(path: &Path) -> io::Result<Option<Self>> {
            match Self::load(path) {
                Ok(m) => Ok(Some(m)),
                Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
                Err(e) => Err(e),
            }
        }

        /// Atomically write the manifest to `path` (write a sibling temp file,
        /// then rename) so a crash mid-write never truncates an existing file.
        pub fn persist(&self, path: &Path) -> io::Result<()> {
            use std::ffi::OsString;
            use std::io::Write;

            if let Some(parent) = path.parent()
                && !parent.as_os_str().is_empty()
            {
                std::fs::create_dir_all(parent)?;
            }
            let json = serde_json::to_string_pretty(self)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

            // Sibling temp path so the rename stays on the same filesystem. The
            // deployer writes one manifest at a time, so a fixed suffix is safe.
            let mut tmp_name = path.file_name().map(OsString::from).unwrap_or_default();
            tmp_name.push(".tmp");
            let tmp_path = path.with_file_name(tmp_name);

            {
                let mut f = std::fs::File::create(&tmp_path)?;
                f.write_all(json.as_bytes())?;
                f.write_all(b"\n")?;
                f.sync_all()?;
            }
            std::fs::rename(&tmp_path, path)?;
            Ok(())
        }
    }
}

#[cfg(feature = "manifest")]
pub use manifest_impl::{ManifestContracts, StellarDeployManifest, Variant};
