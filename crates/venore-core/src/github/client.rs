//! GitHub API client.
//!
//! Wraps `reqwest::Client` with GitHub-specific headers, auth,
//! and rate limit handling. Follows the pattern of `llm/providers/anthropic.rs`.

use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION, USER_AGENT};
use reqwest::{Client, StatusCode};
use serde::de::DeserializeOwned;
use tracing::{debug, warn};

use crate::error::{Result, VenoreError};
use super::types::{GitHubUser, RateLimitInfo};

const API_BASE_URL: &str = "https://api.github.com";
const VENORE_USER_AGENT: &str = "Venore/0.1.0";

/// GitHub API client with authentication and rate limit handling.
pub struct GitHubClient {
    client: Client,
    token: String,
}

impl GitHubClient {
    /// Create a new client with the given access token.
    pub fn new(token: String) -> Self {
        let client = Client::new();
        Self { client, token }
    }

    /// Get the authenticated user (GET /user).
    /// Also serves as token validation.
    pub async fn get_authenticated_user(&self) -> Result<GitHubUser> {
        debug!("Fetching authenticated GitHub user");
        self.get_json("/user").await
    }

    /// Generic GET that deserializes JSON response.
    pub async fn get_json<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let response = self.get(path).await?;
        self.parse_response(response).await
    }

    /// Generic GET with query parameters that deserializes JSON response.
    pub async fn get_json_with_query<T: DeserializeOwned>(
        &self,
        path: &str,
        query: &[(&str, &str)],
    ) -> Result<T> {
        let url = format!("{}{}", API_BASE_URL, path);

        let response = self
            .client
            .get(&url)
            .headers(self.default_headers())
            .query(query)
            .send()
            .await
            .map_err(|e| VenoreError::GitHubApiError {
                status: 0,
                message: format!("Request failed: {}", e),
            })?;

        self.parse_response(response).await
    }

    /// Parse a response: check status, extract rate limit, deserialize JSON.
    async fn parse_response<T: DeserializeOwned>(
        &self,
        response: reqwest::Response,
    ) -> Result<T> {
        let status = response.status();
        let rate_limit = Self::extract_rate_limit(response.headers());
        let body = response.text().await.map_err(|e| VenoreError::GitHubApiError {
            status: 0,
            message: format!("Failed to read response body: {}", e),
        })?;

        if !status.is_success() {
            return Err(Self::map_error(status, &body, rate_limit));
        }

        serde_json::from_str::<T>(&body).map_err(|e| VenoreError::GitHubApiError {
            status: status.as_u16(),
            message: format!("Failed to parse response: {}", e),
        })
    }

    /// Validate the token by calling GET /user.
    /// Returns the user info if valid.
    pub async fn validate_token(&self) -> Result<GitHubUser> {
        self.get_authenticated_user().await
    }

    /// Make an authenticated GET request to the GitHub API.
    async fn get(&self, path: &str) -> Result<reqwest::Response> {
        let url = format!("{}{}", API_BASE_URL, path);

        self.client
            .get(&url)
            .headers(self.default_headers())
            .send()
            .await
            .map_err(|e| VenoreError::GitHubApiError {
                status: 0,
                message: format!("Request failed: {}", e),
            })
    }

    /// Build default headers for GitHub API requests.
    fn default_headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", self.token))
                .unwrap_or_else(|_| HeaderValue::from_static("")),
        );
        headers.insert(
            USER_AGENT,
            HeaderValue::from_static(VENORE_USER_AGENT),
        );
        headers.insert(
            ACCEPT,
            HeaderValue::from_static("application/vnd.github+json"),
        );
        headers.insert(
            "X-GitHub-Api-Version",
            HeaderValue::from_static("2022-11-28"),
        );
        headers
    }

    /// Extract rate limit info from response headers.
    fn extract_rate_limit(headers: &HeaderMap) -> Option<RateLimitInfo> {
        let remaining = headers
            .get("x-ratelimit-remaining")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u32>().ok())?;
        let limit = headers
            .get("x-ratelimit-limit")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u32>().ok())?;
        let reset = headers
            .get("x-ratelimit-reset")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u64>().ok())?;

        Some(RateLimitInfo {
            remaining,
            limit,
            reset,
        })
    }

    /// Map an HTTP error status to a VenoreError.
    fn map_error(status: StatusCode, body: &str, rate_limit: Option<RateLimitInfo>) -> VenoreError {
        // Rate limit exceeded: 403 with remaining=0
        if status == StatusCode::FORBIDDEN {
            if let Some(ref rl) = rate_limit {
                if rl.remaining == 0 {
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_secs())
                        .unwrap_or(0);
                    let reset_seconds = rl.reset.saturating_sub(now);
                    warn!(reset_seconds, "GitHub rate limit exceeded");
                    return VenoreError::GitHubRateLimited { reset_seconds };
                }
            }
        }

        // Unauthorized
        if status == StatusCode::UNAUTHORIZED {
            return VenoreError::GitHubAuthRequired;
        }

        // Extract message from GitHub error response body
        let message = serde_json::from_str::<serde_json::Value>(body)
            .ok()
            .and_then(|v| v.get("message").and_then(|m| m.as_str()).map(String::from))
            .unwrap_or_else(|| body.to_string());

        VenoreError::GitHubApiError {
            status: status.as_u16(),
            message,
        }
    }
}
