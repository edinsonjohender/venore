//! GitHub Pull Requests — list PRs for a repository.

use tracing::debug;

use crate::error::Result;
use super::client::GitHubClient;
use super::types::GitHubPullRequest;

/// List pull requests for a repository.
///
/// Returns `(pull_requests, has_more)`.
/// `has_more` is a heuristic: true when the response contains exactly `per_page` items.
pub async fn list_pull_requests(
    client: &GitHubClient,
    owner: &str,
    repo: &str,
    state: &str,
    page: u32,
    per_page: u32,
) -> Result<(Vec<GitHubPullRequest>, bool)> {
    debug!(owner, repo, state, page, per_page, "Listing pull requests");

    let path = format!("/repos/{}/{}/pulls", owner, repo);
    let page_str = page.to_string();
    let per_page_str = per_page.to_string();

    let prs: Vec<GitHubPullRequest> = client
        .get_json_with_query(
            &path,
            &[
                ("state", state),
                ("per_page", &per_page_str),
                ("page", &page_str),
                ("sort", "updated"),
                ("direction", "desc"),
            ],
        )
        .await?;

    let has_more = prs.len() as u32 == per_page;
    debug!(count = prs.len(), has_more, "Pull requests fetched");

    Ok((prs, has_more))
}

/// Get a single pull request by number.
///
/// Uses the single-PR endpoint which returns the full body and stats
/// (additions, deletions, changed_files) that the list endpoint may truncate.
pub async fn get_pull_request(
    client: &GitHubClient,
    owner: &str,
    repo: &str,
    pr_number: u64,
) -> Result<GitHubPullRequest> {
    debug!(owner, repo, pr_number, "Getting pull request");

    let path = format!("/repos/{}/{}/pulls/{}", owner, repo, pr_number);
    let pr: GitHubPullRequest = client.get_json(&path).await?;

    debug!(pr_number, title = %pr.title, "Pull request fetched");
    Ok(pr)
}
