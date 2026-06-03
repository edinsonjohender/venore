//! GitHub integration — API client, authentication, and repo detection.
//!
//! - **client**: Authenticated reqwest wrapper for GitHub API
//! - **auth**: Device Flow (RFC 8628) + PAT authentication with OS keyring storage
//! - **repo**: Detect GitHub repo from `.git/config` (single source of truth)
//! - **types**: Pure data structures for API responses

pub mod auth;
pub mod branches;
pub mod client;
pub mod clone;
pub mod comments;
pub mod git_auth;
pub mod issues;
pub mod pr_analyzer;
pub mod pr_detail;
pub mod pulls;
pub mod repo;
pub mod repos;
pub mod types;
