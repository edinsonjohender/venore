//! GitHub Branches — list branches for a repository.

use serde::Deserialize;
use tracing::debug;

use crate::error::Result;
use super::client::GitHubClient;

#[derive(Debug, Deserialize)]
pub struct GitHubBranch {
    pub name: String,
}

/// List branch names for a repository (up to 100).
pub async fn list_branches(
    client: &GitHubClient,
    owner: &str,
    repo: &str,
) -> Result<Vec<String>> {
    debug!(owner, repo, "Listing branches");

    let path = format!("/repos/{}/{}/branches", owner, repo);
    let branches: Vec<GitHubBranch> = client
        .get_json_with_query(&path, &[("per_page", "100")])
        .await?;

    Ok(branches.into_iter().map(|b| b.name).collect())
}
