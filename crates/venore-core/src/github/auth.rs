//! GitHub authentication — Device Flow (RFC 8628) + PAT fallback.
//!
//! Token storage uses OS keyring (same pattern as `keyring_store.rs`).

use reqwest::Client;
use tracing::{debug, info, warn};

use crate::error::{Result, VenoreError};
use super::client::GitHubClient;
use super::types::{AccessTokenResponse, DeviceCodeResponse, GitHubUser};

// =============================================================================
// Constants
// =============================================================================

/// GitHub OAuth App client ID (not a secret — public identifier).
/// Replace with your registered OAuth App's client_id.
const GITHUB_CLIENT_ID: &str = "Ov23li0000000000000";

/// Scopes requested during Device Flow.
const GITHUB_SCOPES: &str = "repo read:org";

/// Keyring service name (shared with keyring_store.rs).
const KEYRING_SERVICE: &str = "venore.ai";

/// Keyring entry name for GitHub token.
const KEYRING_GITHUB_KEY: &str = "github_token";

// =============================================================================
// Device Flow
// =============================================================================

/// Step 1: Request a device code from GitHub.
///
/// Returns the device code response with user_code and verification_uri
/// that must be shown to the user.
pub async fn request_device_code() -> Result<DeviceCodeResponse> {
    info!("Starting GitHub Device Flow");

    let client = Client::new();
    let response = client
        .post("https://github.com/login/device/code")
        .header("Accept", "application/json")
        .form(&[
            ("client_id", GITHUB_CLIENT_ID),
            ("scope", GITHUB_SCOPES),
        ])
        .send()
        .await
        .map_err(|e| VenoreError::GitHubDeviceFlowError(format!("Failed to request device code: {}", e)))?;

    let status = response.status();
    let body = response.text().await.map_err(|e| {
        VenoreError::GitHubDeviceFlowError(format!("Failed to read response: {}", e))
    })?;

    if !status.is_success() {
        return Err(VenoreError::GitHubDeviceFlowError(format!(
            "GitHub returned {}: {}",
            status, body
        )));
    }

    serde_json::from_str::<DeviceCodeResponse>(&body).map_err(|e| {
        VenoreError::GitHubDeviceFlowError(format!("Failed to parse device code response: {}", e))
    })
}

/// Step 2: Poll GitHub for an access token.
///
/// Returns:
/// - `Ok(Some(token))` if the user authorized
/// - `Ok(None)` if still pending (authorization_pending / slow_down)
/// - `Err` if expired, access_denied, or other error
pub async fn poll_for_token(device_code: &str, interval: u64) -> Result<Option<String>> {
    debug!("Polling for GitHub access token");

    // Wait the required interval before polling
    tokio::time::sleep(std::time::Duration::from_secs(interval)).await;

    let client = Client::new();
    let response = client
        .post("https://github.com/login/oauth/access_token")
        .header("Accept", "application/json")
        .form(&[
            ("client_id", GITHUB_CLIENT_ID),
            ("device_code", device_code),
            ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
        ])
        .send()
        .await
        .map_err(|e| VenoreError::GitHubDeviceFlowError(format!("Poll request failed: {}", e)))?;

    let body = response.text().await.map_err(|e| {
        VenoreError::GitHubDeviceFlowError(format!("Failed to read poll response: {}", e))
    })?;

    let token_response: AccessTokenResponse = serde_json::from_str(&body).map_err(|e| {
        VenoreError::GitHubDeviceFlowError(format!("Failed to parse token response: {}", e))
    })?;

    // Check if we got a token
    if let Some(token) = token_response.access_token {
        if !token.is_empty() {
            info!("GitHub Device Flow: token received");
            return Ok(Some(token));
        }
    }

    // Check error codes
    match token_response.error.as_deref() {
        Some("authorization_pending") => {
            debug!("Device Flow: authorization pending");
            Ok(None)
        }
        Some("slow_down") => {
            debug!("Device Flow: slow down requested");
            // Extra wait is handled by caller adjusting interval
            Ok(None)
        }
        Some("expired_token") => {
            warn!("Device Flow: device code expired");
            Err(VenoreError::GitHubDeviceFlowError(
                "Device code expired. Please restart the authentication flow.".to_string(),
            ))
        }
        Some("access_denied") => {
            warn!("Device Flow: user denied access");
            Err(VenoreError::GitHubDeviceFlowError(
                "Access denied by user.".to_string(),
            ))
        }
        Some(other) => {
            let desc = token_response
                .error_description
                .unwrap_or_else(|| other.to_string());
            Err(VenoreError::GitHubDeviceFlowError(desc))
        }
        None => {
            // No token and no error — treat as pending
            Ok(None)
        }
    }
}

// =============================================================================
// Token Storage (OS Keyring)
// =============================================================================

/// Store a GitHub token in the OS keyring.
pub fn store_token(token: &str) -> Result<()> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_GITHUB_KEY)
        .map_err(|e| VenoreError::Unknown(format!("Failed to create keyring entry: {}", e)))?;

    entry
        .set_password(token)
        .map_err(|e| VenoreError::Unknown(format!("Failed to store GitHub token: {}", e)))?;

    debug!("GitHub token stored in keyring");
    Ok(())
}

/// Get the stored GitHub token from the OS keyring.
/// Returns None if no token is stored.
pub fn get_stored_token() -> Result<Option<String>> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_GITHUB_KEY)
        .map_err(|e| VenoreError::Unknown(format!("Failed to create keyring entry: {}", e)))?;

    match entry.get_password() {
        Ok(token) => {
            debug!("GitHub token retrieved from keyring");
            Ok(Some(token))
        }
        Err(keyring::Error::NoEntry) => {
            debug!("No GitHub token found in keyring");
            Ok(None)
        }
        Err(e) => Err(VenoreError::Unknown(format!(
            "Failed to retrieve GitHub token: {}",
            e
        ))),
    }
}

/// Remove the stored GitHub token from the OS keyring.
pub fn remove_token() -> Result<()> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_GITHUB_KEY)
        .map_err(|e| VenoreError::Unknown(format!("Failed to create keyring entry: {}", e)))?;

    match entry.delete_credential() {
        Ok(_) => {
            debug!("GitHub token removed from keyring");
            Ok(())
        }
        Err(keyring::Error::NoEntry) => {
            debug!("No GitHub token to remove");
            Ok(())
        }
        Err(e) => Err(VenoreError::Unknown(format!(
            "Failed to remove GitHub token: {}",
            e
        ))),
    }
}

/// Check if a GitHub token is stored.
pub fn has_token() -> Result<bool> {
    Ok(get_stored_token()?.is_some())
}

// =============================================================================
// Git Credential Manager
// =============================================================================

/// Try to extract a GitHub token from Git Credential Manager.
///
/// Runs `git credential fill` with github.com as host.
/// Returns `Ok(Some(token))` if found, `Ok(None)` if not available.
/// Never fails the flow — all errors are swallowed and return `None`.
pub async fn try_git_credential_token() -> Result<Option<String>> {
    let mut child = match crate::utils::quiet_tokio_command("git")
        .args(["credential", "fill"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
    {
        Ok(c) => c,
        Err(_) => return Ok(None), // git not installed or not in PATH
    };

    // Write credential query to stdin
    if let Some(mut stdin) = child.stdin.take() {
        use tokio::io::AsyncWriteExt;
        let _ = stdin
            .write_all(b"protocol=https\nhost=github.com\n\n")
            .await;
    }

    let output = match tokio::time::timeout(
        std::time::Duration::from_secs(5),
        child.wait_with_output(),
    )
    .await
    {
        Ok(Ok(o)) if o.status.success() => o,
        _ => return Ok(None), // timeout, error, or non-zero exit
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        if let Some(password) = line.strip_prefix("password=") {
            let token = password.trim();
            if !token.is_empty() {
                return Ok(Some(token.to_string()));
            }
        }
    }

    Ok(None)
}

/// Resolve GitHub token: keyring first, then Git Credential Manager.
///
/// Does NOT validate the token — the caller is responsible for validation
/// and for storing GCM-sourced tokens in the keyring if valid.
pub async fn resolve_token() -> Result<Option<String>> {
    // 1. Try Venore keyring first (fast, no subprocess)
    if let Some(token) = get_stored_token()? {
        return Ok(Some(token));
    }

    // 2. Try Git Credential Manager
    if let Some(token) = try_git_credential_token().await? {
        info!("GitHub token found via Git Credential Manager");
        return Ok(Some(token));
    }

    Ok(None)
}

// =============================================================================
// PAT Fallback
// =============================================================================

/// Store a Personal Access Token after validating it.
///
/// Calls GET /user to verify the token works, then stores in keyring.
pub async fn store_pat(token: &str) -> Result<GitHubUser> {
    info!("Validating GitHub PAT");

    let client = GitHubClient::new(token.to_string());
    let user = client.validate_token().await?;

    store_token(token)?;
    info!(login = %user.login, "GitHub PAT stored successfully");

    Ok(user)
}
