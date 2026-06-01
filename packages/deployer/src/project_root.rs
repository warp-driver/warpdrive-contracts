//! Project-root operations: read/update the `project_spec_repo` URL.

use warpdrive_client::project_root::ProjectRootClient;
use wasi_soroban_rs::Account;

use crate::config::{NetworkConfig, client_configs};
use crate::error::{DeployerError, Result};
use crate::manifest::{StellarDeployManifest, require_project_root};
use crate::retry::{RetryConfig, retry};
use crate::tx_hash;

/// `get-project-spec-repo`: read the project_spec_repo URL (simulation only).
pub async fn get_project_spec_repo(
    net: &NetworkConfig,
    account: &Account,
    manifest: &StellarDeployManifest,
) -> Result<String> {
    let project_root = require_project_root(manifest)?;
    let configs = client_configs(net, account, project_root)?;
    ProjectRootClient::new(configs)
        .project_spec_repo()
        .await
        .map_err(DeployerError::from)
}

/// `set-project-spec-repo`: update the project_spec_repo URL (admin write).
pub async fn set_project_spec_repo(
    net: &NetworkConfig,
    account: &Account,
    manifest: &StellarDeployManifest,
    repo: &str,
    retry_cfg: RetryConfig,
) -> Result<String> {
    let project_root = require_project_root(manifest)?;
    let configs = client_configs(net, account, project_root)?;

    let resp = retry(retry_cfg, || {
        let configs = configs.clone();
        let repo = repo.to_string();
        async move {
            ProjectRootClient::new(configs)
                .update_project_spec_repo(repo)
                .await
        }
    })
    .await?;

    Ok(tx_hash(&resp))
}
