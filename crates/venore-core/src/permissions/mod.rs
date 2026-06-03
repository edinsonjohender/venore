//! Permission rule engine for AI agent tool calls.
//!
//! Evaluates whether a tool call should be allowed, denied, or require user confirmation.
//! Uses glob pattern matching for resource-specific rules and supports session-scoped approvals.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::tools::names as N;

/// Action to take for a tool call.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PermissionAction {
    /// Allow the tool call without asking.
    Allow,
    /// Ask the user for confirmation before executing.
    Ask,
    /// Deny the tool call outright.
    Deny,
}

/// A single permission rule matching a tool name and optional resource pattern.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionRule {
    /// Tool name to match (e.g. "edit_file", "run_terminal_command", or "*" for all).
    pub tool: String,
    /// Optional glob pattern for the resource (file path, command, URL).
    /// `None` means "match any resource".
    pub pattern: Option<String>,
    /// Action to take when this rule matches.
    pub action: PermissionAction,
}

/// Returns the default permission ruleset.
///
/// - Destructive/external tools require user confirmation (Ask).
/// - Read-only and interaction tools are always allowed.
pub fn default_rules() -> Vec<PermissionRule> {
    vec![
        // ── Tools that require user confirmation ─────────────────────────
        PermissionRule {
            tool: N::RUN_TERMINAL_COMMAND.into(),
            pattern: None,
            action: PermissionAction::Ask,
        },
        PermissionRule {
            tool: N::RUN_APP.into(),
            pattern: None,
            action: PermissionAction::Ask,
        },
        PermissionRule {
            tool: N::WRITE_FILE.into(),
            pattern: None,
            action: PermissionAction::Ask,
        },
        PermissionRule {
            tool: N::EDIT_FILE.into(),
            pattern: None,
            action: PermissionAction::Ask,
        },
        PermissionRule {
            tool: N::WEB_FETCH.into(),
            pattern: None,
            action: PermissionAction::Ask,
        },
        PermissionRule {
            tool: N::WEB_SEARCH.into(),
            pattern: None,
            action: PermissionAction::Ask,
        },
        // ── Read-only tools: always allow ────────────────────────────────
        PermissionRule {
            tool: N::READ_FILE.into(),
            pattern: None,
            action: PermissionAction::Allow,
        },
        PermissionRule {
            tool: N::LIST_FILES.into(),
            pattern: None,
            action: PermissionAction::Allow,
        },
        PermissionRule {
            tool: N::SEARCH_CODE.into(),
            pattern: None,
            action: PermissionAction::Allow,
        },
        PermissionRule {
            tool: N::SEARCH_TEXT.into(),
            pattern: None,
            action: PermissionAction::Allow,
        },
        PermissionRule {
            tool: N::READ_TERMINAL_OUTPUT.into(),
            pattern: None,
            action: PermissionAction::Allow,
        },
        PermissionRule {
            tool: N::CHECK_HEALTH.into(),
            pattern: None,
            action: PermissionAction::Allow,
        },
        // ── Interaction / orchestration tools: always allow ──────────────
        PermissionRule {
            tool: N::ASK_USER.into(),
            pattern: None,
            action: PermissionAction::Allow,
        },
        PermissionRule {
            tool: N::TASK_CREATE.into(),
            pattern: None,
            action: PermissionAction::Allow,
        },
        PermissionRule {
            tool: N::TASK_UPDATE.into(),
            pattern: None,
            action: PermissionAction::Allow,
        },
        PermissionRule {
            tool: N::TASK_LIST.into(),
            pattern: None,
            action: PermissionAction::Allow,
        },
        PermissionRule {
            tool: N::ENTER_PLAN_MODE.into(),
            pattern: None,
            action: PermissionAction::Allow,
        },
        PermissionRule {
            tool: N::SUBMIT_PLAN.into(),
            pattern: None,
            action: PermissionAction::Allow,
        },
        PermissionRule {
            tool: N::SPAWN_AGENT.into(),
            pattern: None,
            action: PermissionAction::Allow,
        },
        // ── File editing (multi) + project query ──────────────────────────
        PermissionRule {
            tool: N::MULTI_EDIT_FILE.into(),
            pattern: None,
            action: PermissionAction::Ask,
        },
        PermissionRule {
            tool: N::ASK_PROJECT.into(),
            pattern: None,
            action: PermissionAction::Allow,
        },
        // ── Logbook tools (read-only): always allow ──────────────────────
        PermissionRule {
            tool: N::LIST_LOGBOOKS.into(),
            pattern: None,
            action: PermissionAction::Allow,
        },
        PermissionRule {
            tool: N::READ_LOGBOOK.into(),
            pattern: None,
            action: PermissionAction::Allow,
        },
        PermissionRule {
            tool: N::SEARCH_LOGBOOK.into(),
            pattern: None,
            action: PermissionAction::Allow,
        },
        PermissionRule {
            tool: N::LIST_CONNECTIONS.into(),
            pattern: None,
            action: PermissionAction::Allow,
        },
        PermissionRule {
            tool: N::LIST_ISLANDS.into(),
            pattern: None,
            action: PermissionAction::Allow,
        },
        PermissionRule {
            tool: N::QUERY_NEIGHBORHOOD.into(),
            pattern: None,
            action: PermissionAction::Allow,
        },
    ]
}

/// Evaluate the permission for a tool call.
///
/// # Arguments
/// - `tool_name` — the tool being invoked (e.g. "edit_file").
/// - `resource` — the primary resource: file path, command string, or URL.
/// - `rules` — ordered list of permission rules to evaluate.
/// - `session_approvals` — set of `"tool_name:resource"` or `"tool_name:*"` keys
///    that were already approved during this session.
///
/// # Resolution order
/// 1. Session approvals cache (exact match or wildcard `tool:*`).
/// 2. Most specific matching rule (exact tool + pattern > exact tool > wildcard `*`).
/// 3. If no rule matches, defaults to `Ask`.
pub fn evaluate(
    tool_name: &str,
    resource: Option<&str>,
    rules: &[PermissionRule],
    session_approvals: &HashSet<String>,
) -> PermissionAction {
    // 1. Find the most specific matching rule
    let mut best_match: Option<&PermissionRule> = None;
    let mut best_specificity: u8 = 0;

    for rule in rules {
        // Check tool name match
        let tool_matches = rule.tool == tool_name || rule.tool == "*";
        if !tool_matches {
            continue;
        }

        // Check pattern match
        let pattern_matches = match (&rule.pattern, resource) {
            (Some(pat), Some(res)) => match glob::Pattern::new(pat) {
                Ok(p) => p.matches(res),
                Err(e) => {
                    tracing::warn!(pattern = %pat, error = %e, "Invalid glob pattern in permission rule, skipping");
                    false
                }
            },
            (None, _) => true,        // no pattern = match all resources
            (Some(_), None) => false,  // pattern requires a resource
        };
        if !pattern_matches {
            continue;
        }

        // Calculate specificity: higher = more specific
        let is_exact_tool = rule.tool == tool_name;
        let specificity = match (is_exact_tool, &rule.pattern) {
            (true, Some(_)) => 3,  // exact tool + pattern
            (true, None) => 2,     // exact tool, any resource
            (false, Some(_)) => 1, // wildcard tool + pattern
            (false, None) => 0,    // wildcard tool, any resource
        };

        if best_match.is_none() || specificity > best_specificity {
            best_match = Some(rule);
            best_specificity = specificity;
        }
    }

    // 2. If the best matching rule is Deny, always enforce it (session approvals cannot override)
    if let Some(rule) = best_match {
        if rule.action == PermissionAction::Deny {
            tracing::debug!(tool = %tool_name, resource = ?resource, "Permission denied by rule");
            return PermissionAction::Deny;
        }
    }

    // 3. Check session approvals cache
    let approval_key = format!("{}:{}", tool_name, resource.unwrap_or("*"));
    if session_approvals.contains(&approval_key)
        || session_approvals.contains(&format!("{}:*", tool_name))
    {
        tracing::debug!(tool = %tool_name, resource = ?resource, "Allowed by session approval");
        return PermissionAction::Allow;
    }

    // 4. Return matched action or default to Ask
    let action = best_match
        .map(|r| r.action.clone())
        .unwrap_or(PermissionAction::Ask);

    tracing::debug!(tool = %tool_name, resource = ?resource, action = ?action, "Permission evaluated");
    action
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_rules_allow_read_tools() {
        let rules = default_rules();
        let approvals = HashSet::new();

        assert_eq!(
            evaluate("read_file", Some("/foo/bar.rs"), &rules, &approvals),
            PermissionAction::Allow
        );
        assert_eq!(
            evaluate("list_files", Some("/src"), &rules, &approvals),
            PermissionAction::Allow
        );
        assert_eq!(
            evaluate("search_code", None, &rules, &approvals),
            PermissionAction::Allow
        );
        assert_eq!(
            evaluate("search_text", None, &rules, &approvals),
            PermissionAction::Allow
        );
        assert_eq!(
            evaluate("read_terminal_output", None, &rules, &approvals),
            PermissionAction::Allow
        );
    }

    #[test]
    fn test_default_rules_ask_for_write_tools() {
        let rules = default_rules();
        let approvals = HashSet::new();

        assert_eq!(
            evaluate("edit_file", Some("/foo/bar.rs"), &rules, &approvals),
            PermissionAction::Ask
        );
        assert_eq!(
            evaluate("write_file", Some("/foo/new.rs"), &rules, &approvals),
            PermissionAction::Ask
        );
        assert_eq!(
            evaluate("run_terminal_command", Some("rm -rf /"), &rules, &approvals),
            PermissionAction::Ask
        );
        assert_eq!(
            evaluate("web_fetch", Some("https://example.com"), &rules, &approvals),
            PermissionAction::Ask
        );
        assert_eq!(
            evaluate("web_search", Some("rust tutorial"), &rules, &approvals),
            PermissionAction::Ask
        );
    }

    #[test]
    fn test_default_rules_allow_interaction_tools() {
        let rules = default_rules();
        let approvals = HashSet::new();

        assert_eq!(
            evaluate("ask_user", None, &rules, &approvals),
            PermissionAction::Allow
        );
        assert_eq!(
            evaluate("task_create", None, &rules, &approvals),
            PermissionAction::Allow
        );
        assert_eq!(
            evaluate("task_update", None, &rules, &approvals),
            PermissionAction::Allow
        );
        assert_eq!(
            evaluate("task_list", None, &rules, &approvals),
            PermissionAction::Allow
        );
        assert_eq!(
            evaluate("enter_plan_mode", None, &rules, &approvals),
            PermissionAction::Allow
        );
        assert_eq!(
            evaluate("submit_plan", None, &rules, &approvals),
            PermissionAction::Allow
        );
        assert_eq!(
            evaluate("spawn_agent", None, &rules, &approvals),
            PermissionAction::Allow
        );
    }

    #[test]
    fn test_unknown_tool_defaults_to_ask() {
        let rules = default_rules();
        let approvals = HashSet::new();

        assert_eq!(
            evaluate("some_unknown_tool", None, &rules, &approvals),
            PermissionAction::Ask
        );
    }

    #[test]
    fn test_session_approval_exact_match() {
        let rules = default_rules();
        let mut approvals = HashSet::new();
        approvals.insert("edit_file:/src/main.rs".to_string());

        // Exact match → Allow
        assert_eq!(
            evaluate("edit_file", Some("/src/main.rs"), &rules, &approvals),
            PermissionAction::Allow
        );
        // Different resource → still Ask
        assert_eq!(
            evaluate("edit_file", Some("/src/lib.rs"), &rules, &approvals),
            PermissionAction::Ask
        );
    }

    #[test]
    fn test_session_approval_wildcard() {
        let rules = default_rules();
        let mut approvals = HashSet::new();
        approvals.insert("edit_file:*".to_string());

        // Wildcard → Allow any resource for that tool
        assert_eq!(
            evaluate("edit_file", Some("/any/file.rs"), &rules, &approvals),
            PermissionAction::Allow
        );
        assert_eq!(
            evaluate("edit_file", None, &rules, &approvals),
            PermissionAction::Allow
        );
        // Different tool → still Ask
        assert_eq!(
            evaluate("write_file", Some("/any/file.rs"), &rules, &approvals),
            PermissionAction::Ask
        );
    }

    #[test]
    fn test_pattern_matching() {
        let rules = vec![
            PermissionRule {
                tool: "edit_file".into(),
                pattern: Some("*.env".into()),
                action: PermissionAction::Deny,
            },
            PermissionRule {
                tool: "edit_file".into(),
                pattern: None,
                action: PermissionAction::Ask,
            },
        ];
        let approvals = HashSet::new();

        // .env files → Deny (pattern match, specificity 3)
        assert_eq!(
            evaluate("edit_file", Some(".env"), &rules, &approvals),
            PermissionAction::Deny
        );
        assert_eq!(
            evaluate("edit_file", Some("production.env"), &rules, &approvals),
            PermissionAction::Deny
        );
        // Non-.env file → Ask (tool default, specificity 2)
        assert_eq!(
            evaluate("edit_file", Some("src/main.rs"), &rules, &approvals),
            PermissionAction::Ask
        );
    }

    #[test]
    fn test_specificity_ordering() {
        let rules = vec![
            // Wildcard catch-all
            PermissionRule {
                tool: "*".into(),
                pattern: None,
                action: PermissionAction::Deny,
            },
            // Tool-level default
            PermissionRule {
                tool: "edit_file".into(),
                pattern: None,
                action: PermissionAction::Ask,
            },
            // Pattern-specific override
            PermissionRule {
                tool: "edit_file".into(),
                pattern: Some("src/**/*.rs".into()),
                action: PermissionAction::Allow,
            },
        ];
        let approvals = HashSet::new();

        // Most specific: exact tool + pattern → Allow
        assert_eq!(
            evaluate("edit_file", Some("src/lib/mod.rs"), &rules, &approvals),
            PermissionAction::Allow
        );
        // Less specific: exact tool, no pattern match → Ask
        assert_eq!(
            evaluate("edit_file", Some("config.toml"), &rules, &approvals),
            PermissionAction::Ask
        );
        // Least specific: wildcard catch-all → Deny
        assert_eq!(
            evaluate("some_other_tool", Some("foo"), &rules, &approvals),
            PermissionAction::Deny
        );
    }

    #[test]
    fn test_deny_rule_overrides_session_approval() {
        let rules = vec![PermissionRule {
            tool: "edit_file".into(),
            pattern: None,
            action: PermissionAction::Deny,
        }];
        let mut approvals = HashSet::new();
        approvals.insert("edit_file:*".to_string());

        // Deny rules cannot be overridden by session approvals
        assert_eq!(
            evaluate("edit_file", Some("any.rs"), &rules, &approvals),
            PermissionAction::Deny
        );
    }

    #[test]
    fn test_session_approval_overrides_ask_rule() {
        let rules = vec![PermissionRule {
            tool: "edit_file".into(),
            pattern: None,
            action: PermissionAction::Ask,
        }];
        let mut approvals = HashSet::new();
        approvals.insert("edit_file:*".to_string());

        // Session approval overrides Ask rules
        assert_eq!(
            evaluate("edit_file", Some("any.rs"), &rules, &approvals),
            PermissionAction::Allow
        );
    }

    #[test]
    fn test_empty_rules_defaults_to_ask() {
        let rules = vec![];
        let approvals = HashSet::new();

        assert_eq!(
            evaluate("anything", None, &rules, &approvals),
            PermissionAction::Ask
        );
    }
}
