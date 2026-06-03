//! Mesh protocol — message types for peer-to-peer communication
//!
//! All messages are JSON-serialized with a `"type"` discriminator field
//! for clean wire format and forward compatibility.

use crate::error::{Result, VenoreError};
use serde::{Deserialize, Serialize};

/// Messages exchanged between mesh peers over WebSocket
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum MeshMessage {
    Ping,
    Pong,
    Disconnect { reason: String },
    AgentRequest {
        stream_id: String,
        from_project: String,
        /// `project_id` of the target peer. The receiving process looks up
        /// the per-project request handler by this key — required since a
        /// single process can host N peers and the request must be routed
        /// to the correct one. `#[serde(default)]` for forward compat:
        /// older callers without this field will hit the inbound handler's
        /// "unknown target project" branch.
        #[serde(default)]
        to_project: String,
        question: String,
        context_hint: Option<String>,
        /// Conversation ID for multi-turn context (Phase 4a).
        /// Old instances without this field will deserialize it as None.
        #[serde(default)]
        conversation_id: Option<String>,
    },
    AgentResponse {
        stream_id: String,
        content: String,
        /// Echo back the conversation_id so the caller can correlate (Phase 4a).
        #[serde(default)]
        conversation_id: Option<String>,
    },
    AgentError {
        stream_id: String,
        error: String,
    },
    /// Follow-up question from the handler sub-agent to the caller (Phase 4b).
    /// Old instances will fail to deserialize this → `Err(_) => continue` ignores it.
    AgentFollowUp {
        stream_id: String,
        question: String,
        round: u32,
    },
    /// Answer to a follow-up question from the caller back to the handler (Phase 4b).
    AgentFollowUpAnswer {
        stream_id: String,
        answer: String,
        round: u32,
    },
}

impl MeshMessage {
    /// Serialize to JSON string
    pub fn to_json(&self) -> Result<String> {
        serde_json::to_string(self).map_err(|e| VenoreError::Json(e.to_string()))
    }

    /// Deserialize from JSON string
    pub fn from_json(json: &str) -> Result<Self> {
        serde_json::from_str(json).map_err(|e| VenoreError::Json(e.to_string()))
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ping_roundtrip() {
        let msg = MeshMessage::Ping;
        let json = msg.to_json().unwrap();
        let parsed = MeshMessage::from_json(&json).unwrap();
        assert!(matches!(parsed, MeshMessage::Ping));
    }

    #[test]
    fn test_pong_roundtrip() {
        let msg = MeshMessage::Pong;
        let json = msg.to_json().unwrap();
        let parsed = MeshMessage::from_json(&json).unwrap();
        assert!(matches!(parsed, MeshMessage::Pong));
    }

    #[test]
    fn test_disconnect_roundtrip() {
        let msg = MeshMessage::Disconnect {
            reason: "shutting down".to_string(),
        };
        let json = msg.to_json().unwrap();
        let parsed = MeshMessage::from_json(&json).unwrap();
        match parsed {
            MeshMessage::Disconnect { reason } => assert_eq!(reason, "shutting down"),
            _ => panic!("Expected Disconnect"),
        }
    }

    #[test]
    fn test_json_has_type_field() {
        let json = MeshMessage::Ping.to_json().unwrap();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(value["type"], "Ping");

        let json = MeshMessage::Pong.to_json().unwrap();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(value["type"], "Pong");

        let json = MeshMessage::Disconnect {
            reason: "bye".to_string(),
        }
        .to_json()
        .unwrap();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(value["type"], "Disconnect");
        assert_eq!(value["reason"], "bye");
    }

    #[test]
    fn test_invalid_json_returns_error() {
        let result = MeshMessage::from_json("not json");
        assert!(result.is_err());
    }

    #[test]
    fn test_agent_request_roundtrip() {
        let msg = MeshMessage::AgentRequest {
            stream_id: "req-123".to_string(),
            from_project: "my-project".to_string(),
            to_project: "target-project".to_string(),
            question: "What is the API shape?".to_string(),
            context_hint: Some("api".to_string()),
            conversation_id: Some("conv-abc".to_string()),
        };
        let json = msg.to_json().unwrap();
        let parsed = MeshMessage::from_json(&json).unwrap();
        match parsed {
            MeshMessage::AgentRequest { stream_id, from_project, to_project, question, context_hint, conversation_id } => {
                assert_eq!(stream_id, "req-123");
                assert_eq!(from_project, "my-project");
                assert_eq!(to_project, "target-project");
                assert_eq!(question, "What is the API shape?");
                assert_eq!(context_hint, Some("api".to_string()));
                assert_eq!(conversation_id, Some("conv-abc".to_string()));
            }
            _ => panic!("Expected AgentRequest"),
        }
    }

    #[test]
    fn test_agent_request_no_hint_roundtrip() {
        let msg = MeshMessage::AgentRequest {
            stream_id: "req-456".to_string(),
            from_project: "proj".to_string(),
            to_project: "target".to_string(),
            question: "How does auth work?".to_string(),
            context_hint: None,
            conversation_id: None,
        };
        let json = msg.to_json().unwrap();
        let parsed = MeshMessage::from_json(&json).unwrap();
        match parsed {
            MeshMessage::AgentRequest { context_hint, conversation_id, .. } => {
                assert_eq!(context_hint, None);
                assert_eq!(conversation_id, None);
            }
            _ => panic!("Expected AgentRequest"),
        }
    }

    #[test]
    fn test_agent_response_roundtrip() {
        let msg = MeshMessage::AgentResponse {
            stream_id: "req-123".to_string(),
            content: "Here is the API shape: ...".to_string(),
            conversation_id: Some("conv-abc".to_string()),
        };
        let json = msg.to_json().unwrap();
        let parsed = MeshMessage::from_json(&json).unwrap();
        match parsed {
            MeshMessage::AgentResponse { stream_id, content, conversation_id } => {
                assert_eq!(stream_id, "req-123");
                assert_eq!(content, "Here is the API shape: ...");
                assert_eq!(conversation_id, Some("conv-abc".to_string()));
            }
            _ => panic!("Expected AgentResponse"),
        }
    }

    #[test]
    fn test_agent_error_roundtrip() {
        let msg = MeshMessage::AgentError {
            stream_id: "req-789".to_string(),
            error: "No handler configured".to_string(),
        };
        let json = msg.to_json().unwrap();
        let parsed = MeshMessage::from_json(&json).unwrap();
        match parsed {
            MeshMessage::AgentError { stream_id, error } => {
                assert_eq!(stream_id, "req-789");
                assert_eq!(error, "No handler configured");
            }
            _ => panic!("Expected AgentError"),
        }
    }

    #[test]
    fn test_agent_request_backward_compat_no_conversation_id() {
        // Simulate JSON from an older instance that doesn't include
        // conversation_id or to_project. Both fields default — to_project
        // becomes "" which triggers the "missing to_project" branch in
        // handle_inbound (intentional: forces older peers to upgrade).
        let json = r#"{"type":"AgentRequest","stream_id":"req-old","from_project":"old-proj","question":"hello?","context_hint":null}"#;
        let parsed = MeshMessage::from_json(json).unwrap();
        match parsed {
            MeshMessage::AgentRequest { stream_id, to_project, conversation_id, .. } => {
                assert_eq!(stream_id, "req-old");
                assert_eq!(to_project, "");
                assert_eq!(conversation_id, None);
            }
            _ => panic!("Expected AgentRequest"),
        }
    }

    #[test]
    fn test_agent_response_backward_compat_no_conversation_id() {
        // Simulate JSON from an older instance that doesn't include conversation_id
        let json = r#"{"type":"AgentResponse","stream_id":"resp-old","content":"some answer"}"#;
        let parsed = MeshMessage::from_json(json).unwrap();
        match parsed {
            MeshMessage::AgentResponse { stream_id, content, conversation_id } => {
                assert_eq!(stream_id, "resp-old");
                assert_eq!(content, "some answer");
                assert_eq!(conversation_id, None);
            }
            _ => panic!("Expected AgentResponse"),
        }
    }

    #[test]
    fn test_agent_follow_up_roundtrip() {
        let msg = MeshMessage::AgentFollowUp {
            stream_id: "req-fu-1".to_string(),
            question: "JWT login or refresh token?".to_string(),
            round: 1,
        };
        let json = msg.to_json().unwrap();
        let parsed = MeshMessage::from_json(&json).unwrap();
        match parsed {
            MeshMessage::AgentFollowUp { stream_id, question, round } => {
                assert_eq!(stream_id, "req-fu-1");
                assert_eq!(question, "JWT login or refresh token?");
                assert_eq!(round, 1);
            }
            _ => panic!("Expected AgentFollowUp"),
        }
    }

    #[test]
    fn test_agent_follow_up_answer_roundtrip() {
        let msg = MeshMessage::AgentFollowUpAnswer {
            stream_id: "req-fu-1".to_string(),
            answer: "Use refresh tokens".to_string(),
            round: 1,
        };
        let json = msg.to_json().unwrap();
        let parsed = MeshMessage::from_json(&json).unwrap();
        match parsed {
            MeshMessage::AgentFollowUpAnswer { stream_id, answer, round } => {
                assert_eq!(stream_id, "req-fu-1");
                assert_eq!(answer, "Use refresh tokens");
                assert_eq!(round, 1);
            }
            _ => panic!("Expected AgentFollowUpAnswer"),
        }
    }

    #[test]
    fn test_agent_request_with_conversation_id_roundtrip() {
        let msg = MeshMessage::AgentRequest {
            stream_id: "req-conv".to_string(),
            from_project: "proj-a".to_string(),
            to_project: "proj-b".to_string(),
            question: "follow-up question".to_string(),
            context_hint: None,
            conversation_id: Some("conv-12345".to_string()),
        };
        let json = msg.to_json().unwrap();
        let parsed = MeshMessage::from_json(&json).unwrap();
        match parsed {
            MeshMessage::AgentRequest { conversation_id, .. } => {
                assert_eq!(conversation_id, Some("conv-12345".to_string()));
            }
            _ => panic!("Expected AgentRequest"),
        }
    }
}
