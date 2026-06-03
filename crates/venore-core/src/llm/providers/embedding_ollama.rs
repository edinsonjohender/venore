//! Ollama Embedding Provider
//!
//! Implements EmbeddingProvider for Ollama local embedding models.
//! Default model: nomic-embed-text (768 dimensions).
//!
//! ## API Documentation
//! - Endpoint: POST http://localhost:11434/api/embed
//! - Docs: https://github.com/ollama/ollama/blob/main/docs/api.md#generate-embeddings

use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::traits::EmbeddingProvider;
use crate::{Result, VenoreError};

const OLLAMA_HOST: &str = "http://localhost:11434";
const DEFAULT_DIMENSIONS: u32 = 768;
const MAX_BATCH_SIZE: usize = 32;

// ============================================================================
// API TYPES
// ============================================================================

#[derive(Serialize)]
struct OllamaEmbedRequest {
    model: String,
    input: Vec<String>,
}

#[derive(Deserialize)]
struct OllamaEmbedResponse {
    embeddings: Vec<Vec<f32>>,
}

// ============================================================================
// PROVIDER
// ============================================================================

pub struct OllamaEmbeddingProvider {
    model: String,
    client: Client,
}

impl OllamaEmbeddingProvider {
    pub fn new(model: String) -> Self {
        Self {
            model,
            client: Client::new(),
        }
    }
}

#[async_trait]
impl EmbeddingProvider for OllamaEmbeddingProvider {
    fn provider_name(&self) -> &str {
        "ollama"
    }

    fn dimensions(&self) -> u32 {
        DEFAULT_DIMENSIONS
    }

    fn model(&self) -> &str {
        &self.model
    }

    async fn embed_batch(&self, _api_key: &str, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        let mut all_embeddings = Vec::with_capacity(texts.len());
        let url = format!("{}/api/embed", OLLAMA_HOST);

        for batch in texts.chunks(MAX_BATCH_SIZE) {
            let request = OllamaEmbedRequest {
                model: self.model.clone(),
                input: batch.to_vec(),
            };

            let response = self.client
                .post(&url)
                .header("Content-Type", "application/json")
                .json(&request)
                .send()
                .await
                .map_err(|e| VenoreError::RagEmbeddingError(format!("Ollama embedding request failed: {}", e)))?;

            if !response.status().is_success() {
                let status = response.status().as_u16();
                let body = response.text().await.unwrap_or_default();
                let body_preview = if body.len() > 200 { &body[..200] } else { &body };
                return Err(VenoreError::RagEmbeddingError(format!(
                    "Ollama embedding error ({}): {}", status, body_preview
                )));
            }

            let data: OllamaEmbedResponse = response.json().await
                .map_err(|e| VenoreError::RagEmbeddingError(format!("Failed to parse Ollama embedding response: {}", e)))?;

            all_embeddings.extend(data.embeddings);
        }

        Ok(all_embeddings)
    }
}
