//! Session Compaction
//!
//! Prevents context overflow during the agentic loop by pruning old tool outputs
//! and, if needed, compacting the entire conversation via an LLM summary.
//!
//! Strategy (2 stages):
//! 1. **Prune** (free, no LLM) — Replace old tool outputs with `[pruned]`, protecting recent ones.
//! 2. **Compact** (LLM call) — Summarize the full conversation if pruning is insufficient.

use crate::llm::gateway::{GatewayOptions, LlmGateway};
use crate::llm::registry::get_model_info;
use crate::llm::types::{LlmMessage, LlmRequest, MessageRole};
use crate::traits::LlmProviderType;
use crate::Result;

// ============================================================================
// CONSTANTS
// ============================================================================

/// Approximate characters per token (heuristic).
const CHARS_PER_TOKEN: u32 = 4;

/// Don't bother pruning if total tokens are below this threshold.
const MIN_TOKENS_FOR_PRUNING: u32 = 20_000;

/// Protect the most recent tool outputs (by token count, scanning from end).
const PROTECT_RECENT_TOKENS: u32 = 40_000;

/// Safety buffer subtracted from usable context to trigger compaction early.
const COMPACTION_BUFFER: u32 = 20_000;

/// Default context window when model info is unavailable.
const DEFAULT_CONTEXT_WINDOW: u32 = 128_000;

/// Default max output tokens when model info is unavailable.
const DEFAULT_MAX_OUTPUT: u32 = 8_192;

// ============================================================================
// TYPES
// ============================================================================

/// Result of a compaction attempt.
#[derive(Debug, Clone)]
pub enum CompactionResult {
    /// No action needed — conversation fits within context.
    NoAction,
    /// Old tool outputs were pruned (replaced with `[pruned]`).
    Pruned { tokens_saved: u32 },
    /// Conversation was summarized by an LLM.
    Compacted {
        original_tokens: u32,
        summary_tokens: u32,
    },
}

/// Internal result of the prune pass.
struct PruneResult {
    pruned: bool,
    tokens_saved: u32,
}

// ============================================================================
// TOKEN ESTIMATION
// ============================================================================

/// Estimate token count for a slice of messages using chars/4 heuristic.
pub fn estimate_tokens(messages: &[LlmMessage]) -> u32 {
    let mut total_chars: usize = 0;

    for msg in messages {
        total_chars += msg.content.len();

        if let Some(ref tool_calls) = msg.tool_calls {
            for tc in tool_calls {
                total_chars += tc.name.len();
                if let Ok(args_str) = serde_json::to_string(&tc.arguments) {
                    total_chars += args_str.len();
                }
            }
        }
    }

    (total_chars / CHARS_PER_TOKEN as usize) as u32
}

/// Check if estimated tokens exceed the usable context window.
pub fn is_overflow(estimated_tokens: u32, context_window: u32, max_output_tokens: u32) -> bool {
    let usable = context_window.saturating_sub(max_output_tokens + COMPACTION_BUFFER);
    estimated_tokens >= usable
}

// ============================================================================
// PRUNE
// ============================================================================

/// Replace old tool outputs with `[pruned]`, protecting the most recent ones.
///
/// Walks messages from end to start. Once `protect_recent_tokens` worth of
/// Tool-role messages have been seen, subsequent Tool messages get their
/// content replaced with `"[pruned]"`.
fn prune_old_tool_outputs(messages: &mut [LlmMessage], protect_recent_tokens: u32) -> PruneResult {
    let total_tokens = estimate_tokens(messages);
    if total_tokens < MIN_TOKENS_FOR_PRUNING {
        return PruneResult {
            pruned: false,
            tokens_saved: 0,
        };
    }

    // Walk backwards, counting Tool tokens. After budget exhausted, prune.
    let mut recent_tool_tokens: u32 = 0;
    let mut tokens_saved: u32 = 0;

    // Collect indices and their token counts first (reverse order)
    let indices_and_tokens: Vec<(usize, u32)> = messages
        .iter()
        .enumerate()
        .rev()
        .filter(|(_, m)| m.role == MessageRole::Tool)
        .map(|(i, m)| (i, m.content.len() as u32 / CHARS_PER_TOKEN))
        .collect();

    for (idx, msg_tokens) in indices_and_tokens {
        if recent_tool_tokens < protect_recent_tokens {
            recent_tool_tokens += msg_tokens;
        } else {
            // This tool output is old enough to prune
            let old_len = messages[idx].content.len() as u32;
            let pruned_len = "[pruned]".len() as u32;
            if old_len > pruned_len {
                messages[idx].content = "[pruned]".to_string();
                tokens_saved += (old_len - pruned_len) / CHARS_PER_TOKEN;
            }
        }
    }

    PruneResult {
        pruned: tokens_saved > 0,
        tokens_saved,
    }
}

// ============================================================================
// COMPACT (LLM SUMMARY)
// ============================================================================

/// Build the compaction prompt that instructs the LLM to summarize.
fn build_compaction_prompt(messages: &[LlmMessage]) -> String {
    let mut conversation = String::new();
    for msg in messages {
        if msg.role == MessageRole::System {
            continue; // System prompt is preserved separately
        }
        let role_str = match msg.role {
            MessageRole::User => "User",
            MessageRole::Assistant => "Assistant",
            MessageRole::Tool => "Tool",
            MessageRole::System => continue,
        };
        conversation.push_str(&format!("[{}]: {}\n\n", role_str, msg.content));
    }

    format!(
        "Summarize the following conversation. Preserve:\n\
         - The current task and what the user wants\n\
         - Key decisions made\n\
         - File changes made (paths and what changed)\n\
         - Errors encountered and how they were resolved\n\
         - Remaining work or next steps\n\n\
         Be concise but complete. Output ONLY the summary.\n\n\
         ---\n\n\
         {conversation}"
    )
}

/// Compact the conversation by summarizing it with an LLM call.
async fn compact(
    gateway: &LlmGateway,
    messages: &[LlmMessage],
    system_prompt: &str,
    options: GatewayOptions,
) -> Result<Vec<LlmMessage>> {
    let compaction_prompt = build_compaction_prompt(messages);

    let request = LlmRequest {
        model: options.model.clone().unwrap_or_else(|| "claude-sonnet-4-5".to_string()),
        messages: vec![LlmMessage {
            role: MessageRole::User,
            content: compaction_prompt,
            tool_call_id: None,
            tool_calls: None,
            content_parts: None,
        }],
        temperature: Some(0.0),
        max_tokens: Some(4096),
        tools: None,
        json_schema: None,
        timeout_secs: Some(60),
        web_search: false,
    };

    let response = tokio::time::timeout(
        std::time::Duration::from_secs(90),
        gateway.complete(request, options),
    )
    .await
    .map_err(|_| crate::error::VenoreError::Timeout(90_000))??;

    // Rebuild messages: system + summary as user context
    Ok(vec![
        LlmMessage {
            role: MessageRole::System,
            content: system_prompt.to_string(),
            tool_call_id: None,
            tool_calls: None,
            content_parts: None,
        },
        LlmMessage {
            role: MessageRole::User,
            content: format!(
                "Previous conversation summary:\n\n{}\n\n\
                 Continue from where we left off.",
                response.content
            ),
            tool_call_id: None,
            tool_calls: None,
            content_parts: None,
        },
    ])
}

// ============================================================================
// ENTRY POINT
// ============================================================================

/// Check if compaction is needed and apply it.
///
/// Called before each `continue_chat_stream()` in the agentic loop.
/// Modifies `messages` in place if pruning/compaction occurs.
pub async fn maybe_compact(
    gateway: &LlmGateway,
    messages: &mut Vec<LlmMessage>,
    provider: LlmProviderType,
    model: &str,
    options: GatewayOptions,
) -> Result<CompactionResult> {
    // 1. Estimate current token usage
    let estimated = estimate_tokens(messages);

    // 2. Get model context window
    let model_info = get_model_info(provider, model);
    let context_window = model_info
        .as_ref()
        .and_then(|i| i.context_window)
        .unwrap_or(DEFAULT_CONTEXT_WINDOW);
    let max_output = model_info
        .as_ref()
        .and_then(|i| i.max_output_tokens)
        .unwrap_or(DEFAULT_MAX_OUTPUT);

    // 3. Check if we're within limits
    if !is_overflow(estimated, context_window, max_output) {
        return Ok(CompactionResult::NoAction);
    }

    tracing::info!(
        estimated_tokens = estimated,
        context_window,
        max_output,
        "Context overflow detected, attempting prune"
    );

    // 4. Try pruning first
    let prune_result = prune_old_tool_outputs(messages, PROTECT_RECENT_TOKENS);
    if prune_result.pruned {
        let after_prune = estimate_tokens(messages);
        if !is_overflow(after_prune, context_window, max_output) {
            tracing::info!(
                tokens_saved = prune_result.tokens_saved,
                tokens_after = after_prune,
                "Prune sufficient — no LLM compaction needed"
            );
            return Ok(CompactionResult::Pruned {
                tokens_saved: prune_result.tokens_saved,
            });
        }
        tracing::info!(
            tokens_after = after_prune,
            "Prune insufficient — proceeding to LLM compaction"
        );
    }

    // 5. Extract system prompt before compaction
    let system_prompt = messages
        .iter()
        .find(|m| m.role == MessageRole::System)
        .map(|m| m.content.clone())
        .unwrap_or_default();

    let original_tokens = estimate_tokens(messages);

    // 6. Full LLM compaction
    let compacted = compact(gateway, messages, &system_prompt, options).await?;
    let summary_tokens = estimate_tokens(&compacted);

    tracing::info!(
        original_tokens,
        summary_tokens,
        "Conversation compacted via LLM"
    );

    *messages = compacted;

    Ok(CompactionResult::Compacted {
        original_tokens,
        summary_tokens,
    })
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn make_msg(role: MessageRole, content: &str) -> LlmMessage {
        let tool_call_id = if matches!(role, MessageRole::Tool) {
            Some("tc_1".to_string())
        } else {
            None
        };
        LlmMessage {
            role,
            content: content.to_string(),
            tool_call_id,
            tool_calls: None,
            content_parts: None,
        }
    }

    #[test]
    fn test_estimate_tokens() {
        // 40 chars / 4 = 10 tokens
        let messages = vec![make_msg(MessageRole::User, "a]".repeat(20).as_str())];
        let tokens = estimate_tokens(&messages);
        assert_eq!(tokens, 10);
    }

    #[test]
    fn test_is_overflow_below_limit() {
        // context_window=100k, max_output=8k, buffer=20k → usable=72k
        // 50k tokens → no overflow
        assert!(!is_overflow(50_000, 100_000, 8_000));
    }

    #[test]
    fn test_is_overflow_above_limit() {
        // context_window=100k, max_output=8k, buffer=20k → usable=72k
        // 80k tokens → overflow
        assert!(is_overflow(80_000, 100_000, 8_000));
    }

    #[test]
    fn test_prune_old_tool_outputs() {
        // Each big_content = 40,000 chars = ~10,000 tokens
        // Total tool tokens = 5 * 10k + small = 50k+ → exceeds MIN_TOKENS_FOR_PRUNING
        // protect_recent_tokens = 5,000
        // Scanning from end (only Tool messages):
        //   idx 9 ("recent"): ~3 tok, cumul 3 < 5000 → protected, cumul = 3
        //   idx 8 (big): 10k tok, cumul 3 < 5000 → protected, cumul = 10,003
        //   idx 5 (big): cumul 10,003 >= 5000 → PRUNED
        //   idx 3 (big): cumul 10,003 >= 5000 → PRUNED
        //   idx 2 (big): cumul 10,003 >= 5000 → PRUNED
        let big_content = "x".repeat(40_000); // ~10,000 tokens each

        let mut messages = vec![
            make_msg(MessageRole::System, "system"),         // 0
            make_msg(MessageRole::User, "hello"),            // 1
            make_msg(MessageRole::Tool, &big_content),       // 2 — pruned
            make_msg(MessageRole::Tool, &big_content),       // 3 — pruned
            make_msg(MessageRole::Assistant, "thinking"),     // 4
            make_msg(MessageRole::Tool, &big_content),       // 5 — pruned
            make_msg(MessageRole::User, "continue"),         // 6
            make_msg(MessageRole::Assistant, "more thinking"),// 7
            make_msg(MessageRole::Tool, &big_content),       // 8 — protected (recent)
            make_msg(MessageRole::Tool, "recent result"),    // 9 — protected (recent)
        ];

        let result = prune_old_tool_outputs(&mut messages, 5_000);
        assert!(result.pruned);
        assert!(result.tokens_saved > 0);

        // Old tool messages should be pruned
        assert_eq!(messages[2].content, "[pruned]");
        assert_eq!(messages[3].content, "[pruned]");
        assert_eq!(messages[5].content, "[pruned]");

        // Non-tool messages untouched
        assert_eq!(messages[0].content, "system");
        assert_eq!(messages[1].content, "hello");
        assert_eq!(messages[4].content, "thinking");

        // Recent tool messages protected (within protect window)
        assert_ne!(messages[8].content, "[pruned]");
        assert_eq!(messages[9].content, "recent result");
    }

    #[test]
    fn test_prune_protects_recent() {
        // Only recent tool messages — all should be protected
        let content = "x".repeat(30_000); // ~7500 tokens each, 3 * 7500 = 22500 < 40000
        let mut messages = vec![
            make_msg(MessageRole::System, "system"),
            make_msg(MessageRole::User, "hello"),
            make_msg(MessageRole::Tool, &content),
            make_msg(MessageRole::Tool, &content),
            make_msg(MessageRole::Tool, &content),
        ];

        let result = prune_old_tool_outputs(&mut messages, 40_000);
        // All 3 tools fit within 40k protect window, nothing pruned
        assert!(!result.pruned);
        assert_eq!(result.tokens_saved, 0);
    }

    #[test]
    fn test_prune_skips_non_tool() {
        let big_content = "x".repeat(100_000); // ~25k tokens
        let mut messages = vec![
            make_msg(MessageRole::System, &big_content),
            make_msg(MessageRole::User, &big_content),
            make_msg(MessageRole::Assistant, &big_content),
        ];

        let original_contents: Vec<String> =
            messages.iter().map(|m| m.content.clone()).collect();

        let result = prune_old_tool_outputs(&mut messages, 40_000);
        // No Tool messages to prune
        assert!(!result.pruned);
        assert_eq!(result.tokens_saved, 0);

        // All content untouched
        for (i, msg) in messages.iter().enumerate() {
            assert_eq!(msg.content, original_contents[i]);
        }
    }

    #[test]
    fn test_prune_minimum_threshold() {
        // Total tokens below MIN_TOKENS_FOR_PRUNING — no prune
        let mut messages = vec![
            make_msg(MessageRole::User, "hello"),
            make_msg(MessageRole::Tool, "short result"),
            make_msg(MessageRole::Assistant, "ok"),
        ];

        let result = prune_old_tool_outputs(&mut messages, 40_000);
        assert!(!result.pruned);
        assert_eq!(result.tokens_saved, 0);
    }
}
