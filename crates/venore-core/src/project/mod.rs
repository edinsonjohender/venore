//! Project Module
//!
//! Stable project identity and registration.
//! - `identity.rs`: ProjectIdentity type stored in `.venore/project.json`
//! - `service.rs`: Read/write `.venore/project.json`
//! - `repository.rs`: SQLite persistence for registered projects

pub mod identity;
pub mod repository;
pub mod service;

pub use identity::{ProjectIdentity, RegisteredProject};
pub use repository::ProjectRepository;
pub use service::ProjectService;
