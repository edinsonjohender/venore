//! Ephemeral authentication for git HTTP operations against GitHub.
//!
//! Authenticating a `git` network operation (clone, fetch, pull) against a
//! private GitHub repo requires a token. The naive approach — injecting the
//! token into the remote URL (`https://x-access-token:TOKEN@github.com/...`) —
//! makes `git` **persist that URL in `.git/config`**, leaving the token in
//! plaintext on disk forever. It also exposes the token to anyone who reads
//! the repo's config.
//!
//! This module provides the correct alternative: pass the token as an
//! ephemeral `http.extraHeader` via git's environment-based config
//! (`GIT_CONFIG_*`, git >= 2.31). The header authenticates the single command
//! and is never written to disk **nor** placed on the process command line
//! (argv). The persisted `origin` URL stays clean
//! (`https://github.com/owner/repo.git`).
//!
//! This is the same mechanism `actions/checkout` uses. Every authenticated git
//! network operation should route through here instead of re-implementing URL
//! injection.

use base64::Engine;

/// Build the environment variables that authenticate a git HTTP operation
/// against GitHub without persisting the token to disk or exposing it in argv.
///
/// Apply them to the `Command` before spawning (works for both
/// `std::process::Command` and `tokio::process::Command`):
///
/// ```ignore
/// let mut cmd = crate::utils::quiet_tokio_command("git");
/// for (k, v) in github_auth_env(token) {
///     cmd.env(k, v);
/// }
/// ```
///
/// Returns an empty vec when `token` is `None` (public repos / unauthenticated
/// operations), so callers can always apply the result unconditionally.
///
/// Uses git's `GIT_CONFIG_COUNT` / `GIT_CONFIG_KEY_n` / `GIT_CONFIG_VALUE_n`
/// mechanism (git >= 2.31) to set `http.extraHeader` for the process only.
pub fn github_auth_env(token: Option<&str>) -> Vec<(String, String)> {
    let Some(token) = token else {
        return Vec::new();
    };

    // GitHub accepts HTTP Basic auth with the username `x-access-token` and the
    // token as the password.
    let credential = base64::engine::general_purpose::STANDARD
        .encode(format!("x-access-token:{}", token));

    vec![
        ("GIT_CONFIG_COUNT".to_string(), "1".to_string()),
        ("GIT_CONFIG_KEY_0".to_string(), "http.extraHeader".to_string()),
        (
            "GIT_CONFIG_VALUE_0".to_string(),
            format!("AUTHORIZATION: basic {}", credential),
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_token_yields_empty_env() {
        assert!(github_auth_env(None).is_empty());
    }

    #[test]
    fn token_sets_extraheader_via_git_config_env() {
        let env = github_auth_env(Some("ghp_secret"));
        let map: std::collections::HashMap<_, _> = env.into_iter().collect();

        assert_eq!(map.get("GIT_CONFIG_COUNT").map(String::as_str), Some("1"));
        assert_eq!(
            map.get("GIT_CONFIG_KEY_0").map(String::as_str),
            Some("http.extraHeader"),
        );

        // base64("x-access-token:ghp_secret")
        let expected = base64::engine::general_purpose::STANDARD
            .encode("x-access-token:ghp_secret");
        assert_eq!(
            map.get("GIT_CONFIG_VALUE_0").map(String::as_str),
            Some(format!("AUTHORIZATION: basic {}", expected).as_str()),
        );
    }

    #[test]
    fn raw_token_never_appears_verbatim() {
        // The token must be base64-wrapped, never present as-is (which would
        // mean it could leak readably through env inspection).
        let env = github_auth_env(Some("ghp_plaintext"));
        for (_, value) in &env {
            assert!(
                !value.contains("ghp_plaintext"),
                "raw token leaked into env value: {value}",
            );
        }
    }
}
