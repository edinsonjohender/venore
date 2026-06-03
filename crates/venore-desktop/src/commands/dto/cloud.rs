//! Cloud auth DTOs — Request/Response types for cloud Tauri commands.

use serde::{Deserialize, Serialize};

// =============================================================================
// Auth
// =============================================================================

/// Response for cloud_auth_status and cloud_get_user.
#[derive(Serialize, Deserialize)]
pub struct CloudAuthStatusResponse {
    pub authenticated: bool,
    pub user_id: Option<String>,
    pub email: Option<String>,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
}

/// Request for cloud_sign_in_with_email.
#[derive(Serialize, Deserialize)]
pub struct CloudSignInWithEmailRequest {
    pub email: String,
    pub password: String,
}

/// Request for cloud_sign_up_with_email.
#[derive(Serialize, Deserialize)]
pub struct CloudSignUpWithEmailRequest {
    pub email: String,
    pub password: String,
    pub display_name: String,
}

/// Response for cloud_sign_up_with_email.
///
/// If Supabase has email confirmation enabled, `needs_confirmation` is true
/// and the user must check their inbox before signing in.
#[derive(Serialize, Deserialize)]
pub struct CloudSignUpResponse {
    pub needs_confirmation: bool,
}

/// User profile returned by the cloud server (GET /users/me).
#[derive(Deserialize)]
pub struct CloudUserProfile {
    pub id: String,
    pub email: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
}

/// Supabase auth token response (POST /auth/v1/token).
#[derive(Deserialize)]
pub struct SupabaseTokenResponse {
    pub access_token: String,
    pub refresh_token: String,
}

/// Supabase auth error response.
#[derive(Deserialize)]
pub struct SupabaseAuthError {
    pub error: Option<String>,
    pub error_description: Option<String>,
    pub msg: Option<String>,
}

/// Supabase sign-up response (POST /auth/v1/signup).
///
/// When auto-confirm is ON: includes access_token + refresh_token.
/// When email confirmation is required: only user data, no tokens.
#[derive(Deserialize)]
pub struct SupabaseSignUpResponse {
    pub access_token: Option<String>,
    pub refresh_token: Option<String>,
}
