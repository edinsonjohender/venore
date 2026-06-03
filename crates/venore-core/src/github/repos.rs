//! GitHub User Repos — list repositories for the authenticated user.

use tracing::debug;

use crate::error::Result;
use super::client::GitHubClient;
use super::types::GitHubUserRepo;

/// List repositories for the authenticated user.
///
/// Returns `(repos, has_more)`.
/// `has_more` is a heuristic: true when the response contains exactly `per_page` items.
pub async fn list_user_repos(
    client: &GitHubClient,
    page: u32,
    per_page: u32,
) -> Result<(Vec<GitHubUserRepo>, bool)> {
    debug!(page, per_page, "Listing user repos");

    let page_str = page.to_string();
    let per_page_str = per_page.to_string();

    let repos: Vec<GitHubUserRepo> = client
        .get_json_with_query(
            "/user/repos",
            &[
                ("sort", "updated"),
                ("direction", "desc"),
                ("affiliation", "owner,collaborator,organization_member"),
                ("per_page", &per_page_str),
                ("page", &page_str),
            ],
        )
        .await?;

    let has_more = repos.len() as u32 == per_page;
    debug!(count = repos.len(), has_more, "User repos fetched");

    Ok((repos, has_more))
}
