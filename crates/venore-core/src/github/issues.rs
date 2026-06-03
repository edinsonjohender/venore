//! GitHub Issues — list issues for a repository (excluding PRs).

use tracing::debug;

use crate::error::Result;
use super::client::GitHubClient;
use super::types::GitHubIssue;

/// List issues for a repository, filtering out pull requests.
///
/// GitHub's `/issues` endpoint returns both issues and PRs.
/// Items with a `pull_request` field are filtered out.
///
/// Returns `(issues, has_more)`.
/// `has_more` is based on the pre-filter count (raw response length == per_page).
pub async fn list_issues(
    client: &GitHubClient,
    owner: &str,
    repo: &str,
    state: &str,
    page: u32,
    per_page: u32,
) -> Result<(Vec<GitHubIssue>, bool)> {
    debug!(owner, repo, state, page, per_page, "Listing issues");

    let path = format!("/repos/{}/{}/issues", owner, repo);
    let page_str = page.to_string();
    let per_page_str = per_page.to_string();

    let items: Vec<GitHubIssue> = client
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

    let has_more = items.len() as u32 == per_page;

    // Filter out PRs (GitHub returns PRs in the /issues endpoint)
    let issues: Vec<GitHubIssue> = items
        .into_iter()
        .filter(|i| i.pull_request.is_none())
        .collect();

    debug!(count = issues.len(), has_more, "Issues fetched (PRs filtered)");

    Ok((issues, has_more))
}
