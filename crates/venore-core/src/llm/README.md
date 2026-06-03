# LLM Module

Generic module for interacting with LLMs (Large Language Models).

## Features

- **Multi-provider**: Anthropic, OpenAI, Gemini
- **Retry & Fallback**: automatic retry logic with exponential backoff
- **Rate limiting**: smart exponential backoff with `Retry-After` header support
- **Session logging**: optional JSONL logging for debugging and metrics
- **Streaming**: streaming-response support
- **Secure storage**: API keys held safely in the OS keyring

## Architecture

```
Gateway (public API)
    ↓
Router (Retry, Fallback, Timeouts)
    ↓
Providers (Anthropic, OpenAI, Gemini)
```

## Modules

- `gateway.rs` — public API of the module
- `router.rs` — retry, fallback, and routing logic
- `providers/` — provider implementations
- `utils/backoff.rs` — exponential backoff system
- `session_logger.rs` — session logging (optional)
- `registry.rs` — registry of supported models
- `config.rs` — LLM task configuration

## Rate limiting & backoff

The exponential backoff follows this formula:
```
delay = base_delay_ms * 2^attempt (capped at 60s)
jitter = ±20% to avoid thundering herd
```

If the server responds with a `Retry-After` header, that exact value is honored.

**Sample progression:**
- Attempt 0: ~1s
- Attempt 1: ~2s
- Attempt 2: ~4s
- Attempt 3: ~8s
- ...
- Cap: 60s

See `utils/backoff.rs` for implementation details.

## Session logging

Optional JSONL logging system for debugging and metrics.

**Activation:**
```bash
export VENORE_SESSION_LOGGING=1
```

**Log location:**
- Windows: `%APPDATA%\venore\sessions\`
- macOS: `~/Library/Application Support/venore/sessions/`
- Linux: `~/.local/share/venore/sessions/`

**Usage:**
```rust
use venore_core::llm::prelude::*;

let logger = SessionLogger::new("my_task");
logger.log(SessionEvent::Started {
    timestamp: chrono::Utc::now().to_rfc3339(),
    task: "analysis".to_string(),
    provider: "anthropic".to_string(),
    model: "claude-sonnet-4-5".to_string(),
}).await?;
```

**Available events:**
- `Started` — session start
- `RequestSent` — request sent
- `ResponseReceived` — response received
- `RetryAttempt` — retry on error
- `FallbackProvider` — provider switch
- `Error` — session error
- `Completed` — session finished

**Analysis with jq:**
```bash
# View every event
cat analysis-{id}.jsonl | jq '.'

# Errors only
cat analysis-{id}.jsonl | jq 'select(.event == "error")'

# Total tokens
cat analysis-{id}.jsonl | jq -s 'map(select(.event == "response_received")) | map(.total_tokens) | add'
```

See `session_logger.rs` for implementation details.

## Testing

```bash
# Every test in the LLM module
cargo test --lib llm::

# Backoff only
cargo test --lib llm::utils::backoff

# Session logger only
cargo test --lib llm::session_logger
```

**Current tests:** 66 passing ✅
