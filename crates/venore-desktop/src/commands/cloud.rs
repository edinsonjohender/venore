//! Tauri commands for Venore Cloud authentication.
//!
//! Optional OAuth flow: browser-based sign in via venore.app,
//! tokens stored in OS keyring, user profile cached in app_settings.
//! 100% optional — the app works fully without authentication.

use tauri::{AppHandle, Emitter};
use tauri_plugin_shell::ShellExt;
use tracing::{debug, info, warn};

use crate::state::LazyAppState;
use crate::utils::{IntoStateCommandResult, StateCommandResult};
use super::dto::cloud::{
    CloudAuthStatusResponse, CloudSignInWithEmailRequest, CloudSignUpResponse,
    CloudSignUpWithEmailRequest, CloudUserProfile, SupabaseAuthError,
    SupabaseSignUpResponse, SupabaseTokenResponse,
};
use venore_core::error::VenoreError;

// =============================================================================
// Constants
// =============================================================================

/// Keyring service name (shared with github/auth.rs).
const KEYRING_SERVICE: &str = "venore.ai";

/// Keyring entry names for cloud tokens.
const KEYRING_CLOUD_ACCESS_TOKEN: &str = "cloud_access_token";
const KEYRING_CLOUD_REFRESH_TOKEN: &str = "cloud_refresh_token";

/// App settings keys for cached user profile.
const SETTING_CLOUD_USER_ID: &str = "cloud_user_id";
const SETTING_CLOUD_EMAIL: &str = "cloud_email";
const SETTING_CLOUD_DISPLAY_NAME: &str = "cloud_display_name";
const SETTING_CLOUD_AVATAR_URL: &str = "cloud_avatar_url";

/// Venore auth page (opens in system browser).
const AUTH_PAGE_URL: &str = "https://venore.app/auth";

/// Venore cloud server base URL.
const VENORE_SERVER_URL: &str = "https://api.venore.app";

/// Supabase project URL (public, not a secret).
const SUPABASE_URL: &str = "https://ijfwmljlsycnrieecgdl.supabase.co";

/// Supabase anon key (public, rate-limited by RLS — not a secret).
const SUPABASE_ANON_KEY: &str = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJpc3MiOiJzdXBhYmFzZSIsInJlZiI6ImlqZndtbGpsc3ljbnJpZWVjZ2RsIiwicm9sZSI6ImFub24iLCJpYXQiOjE3NzE0MTYzMDMsImV4cCI6MjA4Njk5MjMwM30.DMCYJqoY3roDhVH0Xi6c1tx9T9TYcamWafXMyG9AIe4";

// =============================================================================
// Keyring Helpers (same pattern as github/auth.rs)
// =============================================================================

fn keyring_set(key: &str, value: &str) -> Result<(), VenoreError> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, key)
        .map_err(|e| VenoreError::Unknown(format!("Failed to create keyring entry: {}", e)))?;
    entry
        .set_password(value)
        .map_err(|e| VenoreError::Unknown(format!("Failed to store {}: {}", key, e)))?;
    Ok(())
}

fn keyring_get(key: &str) -> Result<Option<String>, VenoreError> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, key)
        .map_err(|e| VenoreError::Unknown(format!("Failed to create keyring entry: {}", e)))?;
    match entry.get_password() {
        Ok(val) => Ok(Some(val)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(VenoreError::Unknown(format!(
            "Failed to retrieve {}: {}",
            key, e
        ))),
    }
}

fn keyring_remove(key: &str) -> Result<(), VenoreError> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, key)
        .map_err(|e| VenoreError::Unknown(format!("Failed to create keyring entry: {}", e)))?;
    match entry.delete_credential() {
        Ok(_) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(VenoreError::Unknown(format!(
            "Failed to remove {}: {}",
            key, e
        ))),
    }
}

// =============================================================================
// Auth Commands
// =============================================================================

/// Check if the user is authenticated with Venore Cloud.
///
/// Reads token from keyring + cached profile from app_settings.
/// Fast, offline, no network call.
#[tauri::command]
pub async fn cloud_auth_status(
    lazy_state: tauri::State<'_, LazyAppState>,
) -> StateCommandResult<CloudAuthStatusResponse> {
    debug!("Checking cloud auth status");

    let result: Result<CloudAuthStatusResponse, VenoreError> = async {
        let token = keyring_get(KEYRING_CLOUD_ACCESS_TOKEN)?;
        if token.is_none() {
            return Ok(CloudAuthStatusResponse {
                authenticated: false,
                user_id: None,
                email: None,
                display_name: None,
                avatar_url: None,
            });
        }

        // Token exists — read cached profile from app_settings
        let config_store = {
            let guard = lazy_state.get();
            guard
                .as_ref()
                .map(|s| std::sync::Arc::clone(&s.config_store))
        };

        let config_store = config_store.ok_or_else(|| {
            VenoreError::Unknown("Backend not initialized".to_string())
        })?;

        let user_id = config_store.get_app_setting(SETTING_CLOUD_USER_ID).await?;
        let email = config_store.get_app_setting(SETTING_CLOUD_EMAIL).await?;
        let display_name = config_store.get_app_setting(SETTING_CLOUD_DISPLAY_NAME).await?;
        let avatar_url = config_store.get_app_setting(SETTING_CLOUD_AVATAR_URL).await?;

        // Filter out empty strings (used as deletion)
        let filter = |v: Option<String>| v.filter(|s| !s.is_empty());

        Ok(CloudAuthStatusResponse {
            authenticated: true,
            user_id: filter(user_id),
            email: filter(email),
            display_name: filter(display_name),
            avatar_url: filter(avatar_url),
        })
    }
    .await;

    result.into_state()
}

/// Start the browser-based OAuth sign-in flow.
///
/// 1. Starts a localhost server via tauri-plugin-oauth
/// 2. Opens system browser to venore.app/auth
/// 3. After user authenticates, callback captures tokens
/// 4. Stores tokens in keyring, fetches user profile
/// 5. Caches profile in app_settings
/// 6. Emits `cloud:auth:success` event
#[tauri::command]
pub async fn cloud_start_sign_in(
    app: AppHandle,
    lazy_state: tauri::State<'_, LazyAppState>,
) -> StateCommandResult<()> {
    info!("Starting cloud sign-in flow");

    let result: Result<(), VenoreError> = async {
        // Get config_store for caching user profile
        let config_store = {
            let guard = lazy_state.get();
            guard
                .as_ref()
                .map(|s| std::sync::Arc::clone(&s.config_store))
        };
        let config_store = config_store.ok_or_else(|| {
            VenoreError::Unknown("Backend not initialized".to_string())
        })?;

        // Capture the tokio runtime handle for the callback closure
        let rt_handle = tokio::runtime::Handle::current();
        let app_clone = app.clone();
        let config_clone = config_store.clone();

        // Start localhost OAuth server
        let port = tauri_plugin_oauth::start_with_config(
            tauri_plugin_oauth::OauthConfig {
                ports: None,
                response: Some(
                    "<html><body style='font-family:system-ui;display:flex;align-items:center;justify-content:center;height:100vh;margin:0;background:#0a0a0a;color:#e5e5e5'><div style='text-align:center'><h2>Signed in!</h2><p>You can close this window and return to Venore.</p></div></body></html>"
                        .into(),
                ),
            },
            move |url| {
                let app = app_clone.clone();
                let config = config_clone.clone();
                let handle = rt_handle.clone();

                handle.spawn(async move {
                    if let Err(e) = handle_oauth_callback(&app, &config, &url).await {
                        warn!("Cloud OAuth callback failed: {}", e);
                        let _ = app.emit(
                            "cloud:auth:error",
                            serde_json::json!({ "reason": e.to_string() }),
                        );
                    }
                });
            },
        )
        .map_err(|e| {
            VenoreError::Unknown(format!("Failed to start OAuth server: {}", e))
        })?;

        info!(port, "OAuth localhost server started");

        // Open system browser to auth page
        let auth_url = format!(
            "{}?redirect_uri=http://localhost:{}/callback",
            AUTH_PAGE_URL, port
        );

        #[allow(deprecated)] // tauri-plugin-opener migration tracked separately
        app.shell()
            .open(&auth_url, None)
            .map_err(|e| {
                VenoreError::Unknown(format!("Failed to open browser: {}", e))
            })?;

        info!("Browser opened to auth page");
        Ok(())
    }
    .await;

    result.into_state()
}

/// Start OAuth sign-in with a specific provider (github, google).
///
/// Opens venore.app/auth with provider + redirect_uri params:
/// 1. Starts localhost server via tauri-plugin-oauth
/// 2. Opens browser to venore.app/auth?provider={p}&redirect_uri=localhost
/// 3. Web page auto-starts OAuth with the given provider
/// 4. After auth, venore.app/auth/callback redirects to localhost with tokens
/// 5. Desktop captures tokens, stores in keyring, fetches profile
/// 6. Emits `cloud:auth:success` event
#[tauri::command]
pub async fn cloud_start_oauth(
    app: AppHandle,
    provider: String,
    lazy_state: tauri::State<'_, LazyAppState>,
) -> StateCommandResult<()> {
    info!(provider = %provider, "Starting OAuth flow via venore.app");

    let result: Result<(), VenoreError> = async {
        let config_store = {
            let guard = lazy_state.get();
            guard
                .as_ref()
                .map(|s| std::sync::Arc::clone(&s.config_store))
        };
        let config_store = config_store.ok_or_else(|| {
            VenoreError::Unknown("Backend not initialized".to_string())
        })?;

        let rt_handle = tokio::runtime::Handle::current();
        let app_clone = app.clone();
        let config_clone = config_store.clone();

        // Start localhost OAuth server — receives tokens from web redirect
        let port = tauri_plugin_oauth::start_with_config(
            tauri_plugin_oauth::OauthConfig {
                ports: None,
                response: Some(
                    "<html><body style='font-family:system-ui;display:flex;align-items:center;justify-content:center;height:100vh;margin:0;background:#0a0a0a;color:#e5e5e5'><div style='text-align:center'><h2>Signed in!</h2><p>You can close this window and return to Venore.</p></div></body></html>"
                        .into(),
                ),
            },
            move |url| {
                let app = app_clone.clone();
                let config = config_clone.clone();
                let handle = rt_handle.clone();

                handle.spawn(async move {
                    if let Err(e) = handle_oauth_callback(&app, &config, &url).await {
                        warn!("Cloud OAuth callback failed: {}", e);
                        let _ = app.emit(
                            "cloud:auth:error",
                            serde_json::json!({ "reason": e.to_string() }),
                        );
                    }
                });
            },
        )
        .map_err(|e| {
            VenoreError::Unknown(format!("Failed to start OAuth server: {}", e))
        })?;

        info!(port, provider = %provider, "OAuth localhost server started");

        // Open browser to venore.app/auth with provider hint
        let auth_url = format!(
            "{}?provider={}&redirect_uri=http://localhost:{}/callback",
            AUTH_PAGE_URL, provider, port
        );

        #[allow(deprecated)]
        app.shell()
            .open(&auth_url, None)
            .map_err(|e| {
                VenoreError::Unknown(format!("Failed to open browser: {}", e))
            })?;

        info!(provider = %provider, "Browser opened to venore.app/auth");
        Ok(())
    }
    .await;

    result.into_state()
}

/// Sign out from Venore Cloud.
///
/// Removes tokens from keyring and cached profile from app_settings.
/// Emits `cloud:auth:signed-out` event.
#[tauri::command]
pub async fn cloud_sign_out(
    app: AppHandle,
    lazy_state: tauri::State<'_, LazyAppState>,
) -> StateCommandResult<()> {
    info!("Signing out from Venore Cloud");

    let result: Result<(), VenoreError> = async {
        // Remove tokens from keyring
        keyring_remove(KEYRING_CLOUD_ACCESS_TOKEN)?;
        keyring_remove(KEYRING_CLOUD_REFRESH_TOKEN)?;

        // Clear cached profile from app_settings (set to empty)
        let config_store = {
            let guard = lazy_state.get();
            guard
                .as_ref()
                .map(|s| std::sync::Arc::clone(&s.config_store))
        };

        if let Some(config) = config_store {
            let _ = config.set_app_setting(SETTING_CLOUD_USER_ID, "").await;
            let _ = config.set_app_setting(SETTING_CLOUD_EMAIL, "").await;
            let _ = config.set_app_setting(SETTING_CLOUD_DISPLAY_NAME, "").await;
            let _ = config.set_app_setting(SETTING_CLOUD_AVATAR_URL, "").await;
        }

        info!("Cloud sign-out complete");

        let _ = app.emit("cloud:auth:signed-out", serde_json::json!({}));

        Ok(())
    }
    .await;

    result.into_state()
}

/// Get cached user profile (fast, offline).
///
/// Subset of cloud_auth_status — reads from app_settings only.
#[tauri::command]
pub async fn cloud_get_user(
    lazy_state: tauri::State<'_, LazyAppState>,
) -> StateCommandResult<CloudAuthStatusResponse> {
    debug!("Getting cached cloud user");

    let result: Result<CloudAuthStatusResponse, VenoreError> = async {
        let token = keyring_get(KEYRING_CLOUD_ACCESS_TOKEN)?;
        if token.is_none() {
            return Ok(CloudAuthStatusResponse {
                authenticated: false,
                user_id: None,
                email: None,
                display_name: None,
                avatar_url: None,
            });
        }

        let config_store = {
            let guard = lazy_state.get();
            guard
                .as_ref()
                .map(|s| std::sync::Arc::clone(&s.config_store))
        };

        let config_store = config_store.ok_or_else(|| {
            VenoreError::Unknown("Backend not initialized".to_string())
        })?;

        let user_id = config_store.get_app_setting(SETTING_CLOUD_USER_ID).await?;
        let email = config_store.get_app_setting(SETTING_CLOUD_EMAIL).await?;
        let display_name = config_store.get_app_setting(SETTING_CLOUD_DISPLAY_NAME).await?;
        let avatar_url = config_store.get_app_setting(SETTING_CLOUD_AVATAR_URL).await?;

        let filter = |v: Option<String>| v.filter(|s| !s.is_empty());

        Ok(CloudAuthStatusResponse {
            authenticated: true,
            user_id: filter(user_id),
            email: filter(email),
            display_name: filter(display_name),
            avatar_url: filter(avatar_url),
        })
    }
    .await;

    result.into_state()
}

/// Sign in with email and password via Supabase Auth.
///
/// POST to Supabase `/auth/v1/token?grant_type=password`, stores tokens in
/// keyring, fetches user profile from cloud server, caches in app_settings.
/// Returns CloudAuthStatusResponse on success.
#[tauri::command]
pub async fn cloud_sign_in_with_email(
    request: CloudSignInWithEmailRequest,
    lazy_state: tauri::State<'_, LazyAppState>,
) -> StateCommandResult<CloudAuthStatusResponse> {
    info!(email = %request.email, "Cloud sign-in with email");

    let result: Result<CloudAuthStatusResponse, VenoreError> = async {
        let client = reqwest::Client::new();

        // Authenticate with Supabase
        let response = client
            .post(format!("{}/auth/v1/token?grant_type=password", SUPABASE_URL))
            .header("apikey", SUPABASE_ANON_KEY)
            .header("Content-Type", "application/json")
            .json(&serde_json::json!({
                "email": request.email,
                "password": request.password,
            }))
            .send()
            .await
            .map_err(|e| {
                VenoreError::Unknown(format!("Failed to contact auth server: {}", e))
            })?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            // Try to parse Supabase error for a better message
            if let Ok(err) = serde_json::from_str::<SupabaseAuthError>(&body) {
                let msg = err
                    .error_description
                    .or(err.msg)
                    .or(err.error)
                    .unwrap_or_else(|| format!("Authentication failed ({})", status));
                return Err(VenoreError::Unknown(msg));
            }
            return Err(VenoreError::Unknown(format!(
                "Auth server returned {}: {}",
                status, body
            )));
        }

        let tokens: SupabaseTokenResponse = response.json().await.map_err(|e| {
            VenoreError::Unknown(format!("Failed to parse auth response: {}", e))
        })?;

        // Store tokens in keyring
        keyring_set(KEYRING_CLOUD_ACCESS_TOKEN, &tokens.access_token)?;
        keyring_set(KEYRING_CLOUD_REFRESH_TOKEN, &tokens.refresh_token)?;
        debug!("Cloud tokens stored in keyring (email auth)");

        // Fetch user profile from cloud server
        let profile = fetch_user_profile(&tokens.access_token).await?;

        // Cache profile in app_settings
        let config_store = {
            let guard = lazy_state.get();
            guard
                .as_ref()
                .map(|s| std::sync::Arc::clone(&s.config_store))
        };
        let config_store = config_store.ok_or_else(|| {
            VenoreError::Unknown("Backend not initialized".to_string())
        })?;

        config_store
            .set_app_setting(SETTING_CLOUD_USER_ID, &profile.id)
            .await?;
        config_store
            .set_app_setting(SETTING_CLOUD_EMAIL, &profile.email)
            .await?;
        config_store
            .set_app_setting(SETTING_CLOUD_DISPLAY_NAME, &profile.display_name)
            .await?;
        config_store
            .set_app_setting(
                SETTING_CLOUD_AVATAR_URL,
                profile.avatar_url.as_deref().unwrap_or(""),
            )
            .await?;

        info!(email = %profile.email, "Cloud email sign-in complete");

        let filter = |v: Option<String>| v.filter(|s| !s.is_empty());

        Ok(CloudAuthStatusResponse {
            authenticated: true,
            user_id: Some(profile.id),
            email: Some(profile.email),
            display_name: Some(profile.display_name),
            avatar_url: filter(profile.avatar_url),
        })
    }
    .await;

    result.into_state()
}

/// Sign up with email and password via Supabase Auth.
///
/// POST to Supabase `/auth/v1/signup`. If email confirmation is required,
/// returns `needs_confirmation: true`. Otherwise auto-logs in.
#[tauri::command]
pub async fn cloud_sign_up_with_email(
    request: CloudSignUpWithEmailRequest,
    lazy_state: tauri::State<'_, LazyAppState>,
) -> StateCommandResult<CloudSignUpResponse> {
    info!(email = %request.email, "Cloud sign-up with email");

    let result: Result<CloudSignUpResponse, VenoreError> = async {
        let client = reqwest::Client::new();

        let response = client
            .post(format!("{}/auth/v1/signup", SUPABASE_URL))
            .header("apikey", SUPABASE_ANON_KEY)
            .header("Content-Type", "application/json")
            .json(&serde_json::json!({
                "email": request.email,
                "password": request.password,
                "data": {
                    "display_name": request.display_name,
                }
            }))
            .send()
            .await
            .map_err(|e| {
                VenoreError::Unknown(format!("Failed to contact auth server: {}", e))
            })?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            if let Ok(err) = serde_json::from_str::<SupabaseAuthError>(&body) {
                let msg = err
                    .error_description
                    .or(err.msg)
                    .or(err.error)
                    .unwrap_or_else(|| format!("Sign up failed ({})", status));
                return Err(VenoreError::Unknown(msg));
            }
            return Err(VenoreError::Unknown(format!(
                "Auth server returned {}: {}",
                status, body
            )));
        }

        let signup: SupabaseSignUpResponse = response.json().await.map_err(|e| {
            VenoreError::Unknown(format!("Failed to parse signup response: {}", e))
        })?;

        // If tokens present → auto-confirm is ON → log in immediately
        if let (Some(access_token), Some(refresh_token)) =
            (&signup.access_token, &signup.refresh_token)
        {
            keyring_set(KEYRING_CLOUD_ACCESS_TOKEN, access_token)?;
            keyring_set(KEYRING_CLOUD_REFRESH_TOKEN, refresh_token)?;

            // Fetch + cache profile
            let profile = fetch_user_profile(access_token).await?;

            let config_store = {
                let guard = lazy_state.get();
                guard
                    .as_ref()
                    .map(|s| std::sync::Arc::clone(&s.config_store))
            };
            if let Some(config) = config_store {
                let _ = config
                    .set_app_setting(SETTING_CLOUD_USER_ID, &profile.id)
                    .await;
                let _ = config
                    .set_app_setting(SETTING_CLOUD_EMAIL, &profile.email)
                    .await;
                let _ = config
                    .set_app_setting(SETTING_CLOUD_DISPLAY_NAME, &profile.display_name)
                    .await;
                let _ = config
                    .set_app_setting(
                        SETTING_CLOUD_AVATAR_URL,
                        profile.avatar_url.as_deref().unwrap_or(""),
                    )
                    .await;
            }

            info!(email = %profile.email, "Cloud sign-up + auto-login complete");
            return Ok(CloudSignUpResponse {
                needs_confirmation: false,
            });
        }

        // No tokens → email confirmation required
        info!(email = %request.email, "Cloud sign-up complete, email confirmation required");
        Ok(CloudSignUpResponse {
            needs_confirmation: true,
        })
    }
    .await;

    result.into_state()
}

// =============================================================================
// Internal: OAuth Callback Handler
// =============================================================================

/// Handles the OAuth callback URL from the localhost server.
///
/// Parses tokens from query params, stores in keyring, fetches user profile
/// from the cloud server, caches in app_settings, and emits success event.
async fn handle_oauth_callback(
    app: &AppHandle,
    config_store: &venore_core::infrastructure::config::DefaultConfigStore,
    url: &str,
) -> Result<(), VenoreError> {
    info!("Processing OAuth callback");

    // Parse tokens from callback URL query params
    let parsed = url::Url::parse(url).map_err(|e| {
        VenoreError::Unknown(format!("Failed to parse callback URL: {}", e))
    })?;

    let params: std::collections::HashMap<String, String> =
        parsed.query_pairs().map(|(k, v)| (k.to_string(), v.to_string())).collect();

    let access_token = params.get("access_token").ok_or_else(|| {
        VenoreError::Unknown("No access_token in callback URL".to_string())
    })?;

    let refresh_token = params.get("refresh_token");

    // Store tokens in keyring
    keyring_set(KEYRING_CLOUD_ACCESS_TOKEN, access_token)?;
    if let Some(rt) = refresh_token {
        keyring_set(KEYRING_CLOUD_REFRESH_TOKEN, rt)?;
    }
    debug!("Cloud tokens stored in keyring");

    // Fetch user profile from cloud server
    let profile = fetch_user_profile(access_token).await?;

    // Cache profile in app_settings
    config_store
        .set_app_setting(SETTING_CLOUD_USER_ID, &profile.id)
        .await?;
    config_store
        .set_app_setting(SETTING_CLOUD_EMAIL, &profile.email)
        .await?;
    config_store
        .set_app_setting(SETTING_CLOUD_DISPLAY_NAME, &profile.display_name)
        .await?;
    config_store
        .set_app_setting(
            SETTING_CLOUD_AVATAR_URL,
            profile.avatar_url.as_deref().unwrap_or(""),
        )
        .await?;

    info!(
        email = %profile.email,
        "Cloud sign-in complete"
    );

    // Emit success event
    let _ = app.emit(
        "cloud:auth:success",
        serde_json::json!({
            "user_id": profile.id,
            "email": profile.email,
            "display_name": profile.display_name,
            "avatar_url": profile.avatar_url,
        }),
    );

    Ok(())
}

/// Fetch user profile from the Venore cloud server.
///
/// GET /users/me with Bearer token. Auto-creates user on first login.
async fn fetch_user_profile(access_token: &str) -> Result<CloudUserProfile, VenoreError> {
    let client = reqwest::Client::new();
    let response = client
        .get(format!("{}/users/me", VENORE_SERVER_URL))
        .header("Authorization", format!("Bearer {}", access_token))
        .send()
        .await
        .map_err(|e| {
            VenoreError::Unknown(format!("Failed to fetch user profile: {}", e))
        })?;

    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(VenoreError::Unknown(format!(
            "Cloud server returned {}: {}",
            status, body
        )));
    }

    response.json::<CloudUserProfile>().await.map_err(|e| {
        VenoreError::Unknown(format!("Failed to parse user profile: {}", e))
    })
}
