//! # Mesh Module
//!
//! Peer discovery and transport for multi-instance communication.
//! Instances register themselves in `~/.venore/mesh/` and discover
//! other running instances by reading that directory. WebSocket
//! transport enables bidirectional message exchange between peers.
//!
//! ## Lifecycle
//!
//! The entire mesh lifecycle is managed atomically via `lifecycle::mesh_init()`
//! and `lifecycle::mesh_stop()`. Frontend calls a single command; backend
//! handles register → transport → handler → auto-connect → background loop.

pub mod agent_loop;
mod discovery;
mod handler;
pub mod lifecycle;
mod protocol;
mod transport;
mod types;

/// TTL for conversation entries — 10 minutes of inactivity.
/// Shared between transport (caller-side) and handler (responder-side).
pub const MESH_CONVERSATION_TTL_SECS: u64 = 600;

pub use discovery::MeshDiscovery;
pub use handler::{AgentHandler, MeshRequestHandler};
pub use lifecycle::MeshEventEmitter;
pub use protocol::MeshMessage;
pub use transport::{
    MeshTransport, CallerMessage, set_request_handler, unset_request_handler,
    clear_request_handlers, remove_pending_response,
    get_or_create_conversation_id,
};
pub use types::{PeerInfo, PeerRegistration, ProjectProfile};
