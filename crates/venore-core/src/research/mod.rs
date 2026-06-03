//! Research Engine Module
//!
//! Multi-agent orchestration engine for knowledge investigation.
//! Deploys a Manager (LLM-driven) + parallel Worker agents,
//! each with their own agentic loop and tools.

pub mod types;
pub mod repository;
pub mod worker;
pub mod prompts;
pub mod manager;
pub mod engine;

pub use types::{
    ResearchPhase, ResearchStatus, ResearchRun, ResearchEvent, ResearchEventEmitter,
    WorkerAssignment, WorkerResult, max_workers_for_intensity,
};
pub use repository::ResearchRepository;
pub use worker::run_research_worker;
pub use engine::ResearchEngine;
