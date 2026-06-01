//! Identity handling: BYOK secret resolution, keyfile read/write, and the
//! `keygen` generate+friendbot-fund+persist flow.

use std::path::{Path, PathBuf};

use ed25519_dalek::SigningKey;
use wasi_soroban_rs::wasi_stellar_rpc_client::Client;
use wasi_soroban_rs::{Account, Signer};

use crate::config::NetworkConfig;
use crate::error::{DeployerError, Result};
use crate::retry::{RetryConfig, retry};

/// Env var carrying a BYOK secret (`S…`).
pub const DEPLOYER_SECRET_ENV: &str = "DEPLOYER_SECRET";
/// Default keyfile the wrapper writes via `keygen` and the other commands read.
pub const DEFAULT_KEY_FILE: &str = "/out/.keys/deployer.secret";
/// How long to wait for a friendbot-funded account to become visible on RPC.
const ACCOUNT_PROPAGATE_TIMEOUT_SECS: u64 = 60;

/// Build a single-signer `Account` from a Stellar secret (`S…`).
pub fn account_from_secret(secret: &str) -> Result<Account> {
    let pk = stellar_strkey::ed25519::PrivateKey::from_string(secret.trim()).map_err(|e| {
        DeployerError::Identity(format!("not a valid Stellar secret (S...): {e:?}"))
    })?;
    Ok(Account::single(Signer::new(SigningKey::from_bytes(&pk.0))))
}

/// Resolve a BYOK secret following the precedence in PLAN.md §6:
/// `--secret` → `--secret-file` → `DEPLOYER_SECRET` → default keyfile.
pub fn resolve_secret(
    secret_flag: Option<String>,
    secret_file_flag: Option<PathBuf>,
) -> Result<String> {
    if let Some(s) = secret_flag {
        return Ok(s.trim().to_string());
    }
    if let Some(path) = secret_file_flag {
        return read_key_file(&path);
    }
    if let Ok(s) = std::env::var(DEPLOYER_SECRET_ENV)
        && !s.trim().is_empty()
    {
        return Ok(s.trim().to_string());
    }
    let default = Path::new(DEFAULT_KEY_FILE);
    if default.exists() {
        return read_key_file(default);
    }
    Err(DeployerError::Identity(format!(
        "no deployer secret found: pass --secret/--secret-file, set {DEPLOYER_SECRET_ENV}, \
         or run `keygen` to create {DEFAULT_KEY_FILE}"
    )))
}

/// Resolve a BYOK secret and turn it into an `Account`.
pub fn resolve_account(
    secret_flag: Option<String>,
    secret_file_flag: Option<PathBuf>,
) -> Result<Account> {
    let secret = resolve_secret(secret_flag, secret_file_flag)?;
    account_from_secret(&secret)
}

/// Read a trimmed secret string from a keyfile.
pub fn read_key_file(path: &Path) -> Result<String> {
    let raw = std::fs::read_to_string(path)?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(DeployerError::Identity(format!(
            "keyfile {} is empty",
            path.display()
        )));
    }
    Ok(trimmed.to_string())
}

/// Write a secret to a keyfile with `0600` permissions, creating parent dirs.
pub fn write_key_file(path: &Path, secret: &str) -> Result<()> {
    use std::io::Write;

    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)?;
    }

    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let mut file = opts.open(path)?;
    file.write_all(secret.as_bytes())?;
    file.write_all(b"\n")?;
    Ok(())
}

/// Generate a fresh ed25519 secret strkey (`S…`).
fn generate_secret() -> String {
    use rand::RngCore;
    let mut bytes = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    format!("{}", stellar_strkey::ed25519::PrivateKey(bytes))
}

/// `keygen`: ensure `key_file` holds a funded identity, generating + funding one
/// if needed. Idempotent — a re-run with an already-funded keyfile is a no-op
/// beyond re-deriving the address. Returns the `G…` address.
pub async fn keygen_and_fund(
    net: &NetworkConfig,
    friendbot: Option<&str>,
    key_file: &Path,
    retry_cfg: RetryConfig,
) -> Result<String> {
    // Reuse an existing valid keyfile; otherwise mint a new secret.
    let secret = match read_key_file(key_file) {
        Ok(existing) if account_from_secret(&existing).is_ok() => existing,
        _ => {
            let fresh = generate_secret();
            write_key_file(key_file, &fresh)?;
            fresh
        }
    };

    let account = account_from_secret(&secret)?;
    let address = account.account_id().to_string();

    if account_visible(net, &address).await {
        return Ok(address);
    }

    let friendbot_url = match friendbot {
        Some(url) => url.to_string(),
        None => {
            let client = Client::new(&net.rpc_url)
                .map_err(|e| DeployerError::Http(format!("rpc client: {e}")))?;
            client
                .friendbot_url()
                .await
                .map_err(|e| DeployerError::Http(format!("could not derive friendbot url: {e}")))?
        }
    };

    fund_via_friendbot(&friendbot_url, &address, retry_cfg).await?;
    wait_for_account(net, &address).await?;
    Ok(address)
}

/// Whether the account is currently visible to the RPC (i.e. funded).
async fn account_visible(net: &NetworkConfig, address: &str) -> bool {
    match Client::new(&net.rpc_url) {
        Ok(client) => client.get_account(address).await.is_ok(),
        Err(_) => false,
    }
}

/// GET the friendbot endpoint to fund `address`.
async fn fund_via_friendbot(
    friendbot_url: &str,
    address: &str,
    retry_cfg: RetryConfig,
) -> Result<()> {
    let url = format!("{friendbot_url}?addr={address}");
    retry(retry_cfg, || {
        let url = url.clone();
        async move {
            let resp = reqwest::get(&url)
                .await
                .map_err(|e| DeployerError::Http(format!("friendbot request failed: {e}")))?;
            if resp.status().is_success() {
                Ok(())
            } else {
                Err(DeployerError::Http(format!(
                    "friendbot returned status {}",
                    resp.status()
                )))
            }
        }
    })
    .await
}

/// Poll until the account is visible on RPC, or time out.
async fn wait_for_account(net: &NetworkConfig, address: &str) -> Result<()> {
    let attempts = ACCOUNT_PROPAGATE_TIMEOUT_SECS / 2;
    for _ in 0..attempts {
        if account_visible(net, address).await {
            return Ok(());
        }
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    }
    Err(DeployerError::Http(format!(
        "account {address} did not become visible within {ACCOUNT_PROPAGATE_TIMEOUT_SECS}s"
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The derivation `S… → G…` is locked: deriving the address two independent
    /// ways agrees.
    #[test]
    fn secret_to_address_is_stable() {
        let mut bytes = [0u8; 32];
        bytes[31] = 7;
        let secret = format!("{}", stellar_strkey::ed25519::PrivateKey(bytes));

        let account = account_from_secret(&secret).unwrap();
        let derived = account.account_id().to_string();

        let signing = SigningKey::from_bytes(&bytes);
        let expected = format!(
            "{}",
            stellar_strkey::ed25519::PublicKey(signing.verifying_key().to_bytes())
        );
        assert_eq!(derived, expected);
        assert!(derived.starts_with('G'));
    }

    #[test]
    fn rejects_non_secret() {
        assert!(account_from_secret("not-a-secret").is_err());
    }

    #[test]
    fn keyfile_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("deployer.secret");
        let secret = generate_secret();
        write_key_file(&path, &secret).unwrap();

        let read_back = read_key_file(&path).unwrap();
        assert_eq!(read_back, secret);
        // The written secret is a valid identity.
        assert!(account_from_secret(&read_back).is_ok());

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }
    }
}
