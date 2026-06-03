//! GitHub comments — issue comments and PR review comments.

use tracing::debug;

use crate::error::Result;
use super::client::GitHubClient;
use super::types::{GitHubComment, GitHubReviewComment};

/// List comments on an issue or PR (general discussion comments).
///
/// Endpoint: GET /repos/{owner}/{repo}/issues/{number}/comments
pub async fn list_issue_comments(
    client: &GitHubClient,
    owner: &str,
    repo: &str,
    number: u64,
) -> Result<Vec<GitHubComment>> {
    debug!(owner, repo, number, "Listing issue comments");

    let path = format!("/repos/{}/{}/issues/{}/comments", owner, repo, number);

    let comments: Vec<GitHubComment> = client.get_json(&path).await?;

    debug!(count = comments.len(), "Issue comments fetched");

    Ok(comments)
}

/// List inline review comments on a PR.
///
/// Endpoint: GET /repos/{owner}/{repo}/pulls/{pr_number}/comments
pub async fn list_pr_review_comments(
    client: &GitHubClient,
    owner: &str,
    repo: &str,
    pr_number: u64,
) -> Result<Vec<GitHubReviewComment>> {
    debug!(owner, repo, pr_number, "Listing PR review comments");

    let path = format!("/repos/{}/{}/pulls/{}/comments", owner, repo, pr_number);

    let comments: Vec<GitHubReviewComment> = client.get_json(&path).await?;

    debug!(count = comments.len(), "PR review comments fetched");

    Ok(comments)
}
