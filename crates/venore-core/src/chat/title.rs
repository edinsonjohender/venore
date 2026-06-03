//! Session title generation via LLM.
//!
//! Generates a concise title for a chat session based on the first user message.

use crate::llm::gateway::{GatewayOptions, LlmGateway};
use crate::llm::types::{LlmMessage, LlmRequest, MessageRole};
use crate::traits::{LlmProviderType, LlmTask};
use crate::Result;

/// Generate a short session title from the first user message.
///
/// Uses a lightweight LLM call (low max_tokens, low temperature) to produce
/// a concise 3-6 word title suitable for a tab label.
pub async fn generate_session_title(
    gateway: &LlmGateway,
    user_message: &str,
    provider: LlmProviderType,
    model: &str,
) -> Result<String> {
    let request = LlmRequest {
        model: model.to_string(),
        messages: vec![
            LlmMessage {
                role: MessageRole::System,
                content: "You generate short titles for chat conversations. \
                          Respond with ONLY the title, 3-6 words, no quotes, no extra text."
                    .to_string(),
                tool_call_id: None,
                tool_calls: None,
                content_parts: None,
            },
            LlmMessage {
                role: MessageRole::User,
                content: user_message.to_string(),
                tool_call_id: None,
                tool_calls: None,
                content_parts: None,
            },
        ],
        temperature: Some(0.3),
        max_tokens: Some(100),
        tools: None,
        json_schema: None,
        timeout_secs: Some(15),
        web_search: false,
    };

    let options = GatewayOptions::for_task(LlmTask::Chat)
        .with_provider(provider)
        .with_model(model);

    tracing::debug!(
        provider = provider.as_str(),
        model = model,
        "Generating session title for: {}",
        &user_message[..user_message.len().min(80)]
    );

    let response = gateway.complete(request, options).await?;

    tracing::info!(
        provider = response.provider.as_str(),
        model = %response.model,
        content_len = response.content.len(),
        "Title generation response: {:?}",
        &response.content
    );

    let title = response.content.trim().to_string();
    if title.is_empty() {
        return Err(crate::VenoreError::LlmInvalidResponse(
            "Title generation returned empty content".into(),
        ));
    }

    Ok(title)
}
