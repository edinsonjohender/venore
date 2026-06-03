//! # Venore REST API Server
//!
//! HTTP server that exposes venore-core functionality
//! as a REST API for web and mobile clients.
//!
//! ## Endpoints:
//!
//! - `GET /health` - Health check
//! - `POST /api/projects/analyze` - Analyze a project
//! - `GET /api/projects/:id` - Fetch a project by ID
//! - `GET /api/projects` - List projects
//! - `POST /api/context/generate` - Generate context with the LLM
//!
//! ## Usage
//!
//! ```bash
//! cargo run -p venore-api
//! # Server running on http://localhost:3000
//! ```

use axum::{
    Router,
    routing::{get, post},
    response::Json,
};
use serde::Serialize;
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tracing::info;

mod routes;
mod handlers;

// ============================================================================
// APP STATE
// ============================================================================

/// Shared application state
#[derive(Clone)]
struct AppState {
    // Injected dependencies would go here:
    // - Database pool
    // - Repository instances
    // - Service instances
}

impl AppState {
    fn new() -> Self {
        Self {}
    }
}

// ============================================================================
// MAIN
// ============================================================================

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_target(false)
        .compact()
        .init();

    info!("Starting Venore API server...");

    // Build the app state
    let state = Arc::new(AppState::new());

    // Build the router
    let app = Router::new()
        // Health check
        .route("/health", get(health_handler))

        // API routes
        .route("/api/projects/analyze", post(routes::projects::analyze))
        .route("/api/projects/:id", get(routes::projects::get_by_id))
        .route("/api/projects", get(routes::projects::list))

        // Context
        .route("/api/context/generate", post(routes::context::generate))

        // State and middleware
        .with_state(state)
        .layer(CorsLayer::permissive()); // TODO: configure CORS properly

    // Start the server
    let addr = "127.0.0.1:3000";
    let listener = tokio::net::TcpListener::bind(addr).await?;

    info!("Server running on http://{}", addr);
    info!("Health check: http://{}/health", addr);

    axum::serve(listener, app).await?;

    Ok(())
}

// ============================================================================
// HANDLERS
// ============================================================================

/// Health check endpoint
async fn health_handler() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

#[derive(Serialize)]
struct HealthResponse {
    status: String,
    version: String,
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_health_endpoint() {
        let response = health_handler().await;
        assert_eq!(response.0.status, "ok");
    }
}
