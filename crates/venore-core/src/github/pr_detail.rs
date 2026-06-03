//! GitHub PR detail — list files changed in a pull request.

use tracing::debug;

use crate::error::Result;
use super::client::GitHubClient;
use super::types::GitHubPullRequestFile;

/// List files changed in a pull request.
///
/// Endpoint: GET /repos/{owner}/{repo}/pulls/{pr_number}/files
pub async fn list_pr_files(
    client: &GitHubClient,
    owner: &str,
    repo: &str,
    pr_number: u64,
    page: u32,
    per_page: u32,
) -> Result<Vec<GitHubPullRequestFile>> {
    debug!(owner, repo, pr_number, page, per_page, "Listing PR files");

    let path = format!("/repos/{}/{}/pulls/{}/files", owner, repo, pr_number);
    let page_str = page.to_string();
    let per_page_str = per_page.to_string();

    let files: Vec<GitHubPullRequestFile> = client
        .get_json_with_query(
            &path,
            &[
                ("per_page", &per_page_str),
                ("page", &page_str),
            ],
        )
        .await?;

    debug!(count = files.len(), "PR files fetched");

    Ok(files)
}
