//! OpenAI Embedding Provider
//!
//! Implements EmbeddingProvider for OpenAI's text-embedding models.
//! Default model: text-embedding-3-small (1536 dimensions).
//!
//! ## API Documentation
//! - Endpoint: POST https://api.openai.com/v1/embeddings
//! - Docs: https://platform.openai.com/docs/api-reference/embeddings

use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::traits::EmbeddingProvider;
use crate::{Result, VenoreError};

const API_URL: &str = "https://api.openai.com/v1/embeddings";
const DEFAULT_MODEL: &str = "text-embedding-3-small";
const DEFAULT_DIMENSIONS: u32 = 1536;
const MAX_BATCH_SIZE: usize = 100;

// ============================================================================
// API TYPES
// ============================================================================

#[derive(Serialize)]
struct EmbeddingRequest {
    model: String,
    input: Vec<String>,
}

#[derive(Deserialize)]
struct EmbeddingResponse {
    data: Vec<EmbeddingData>,
}

#[derive(Deserialize)]
struct EmbeddingData {
    embedding: Vec<f32>,
}

// ============================================================================
// PROVIDER
// ============================================================================

pub struct OpenAIEmbeddingProvider {
    model: String,
    client: Client,
}

impl OpenAIEmbeddingProvider {
    pub fn new(model: String) -> Self {
        Self {
            model,
            client: Client::new(),
        }
    }
}

#[async_trait]
impl EmbeddingProvider for OpenAIEmbeddingProvider {
    fn provider_name(&self) -> &str {
        "openai"
    }

    fn dimensions(&self) -> u32 {
        if self.model == DEFAULT_MODEL {
            DEFAULT_DIMENSIONS
        } else {
            DEFAULT_DIMENSIONS // Most OpenAI embedding models use 1536
        }
    }

    fn model(&self) -> &str {
        &self.model
    }

    async fn embed_batch(&self, api_key: &str, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        let mut all_embeddings = Vec::with_capacity(texts.len());

        // Process in batches of MAX_BATCH_SIZE
        for batch in texts.chunks(MAX_BATCH_SIZE) {
            let request = EmbeddingRequest {
                model: self.model.clone(),
                input: batch.to_vec(),
            };

            let response = self.client
                .post(API_URL)
                .header("Authorization", format!("Bearer {}", api_key))
                .header("Content-Type", "application/json")
                .json(&request)
                .send()
                .await
                .map_err(|e| VenoreError::RagEmbeddingError(format!("OpenAI embedding request failed: {}", e)))?;

            if !response.status().is_success() {
                let status = response.status().as_u16();
                let body = response.text().await.unwrap_or_default();
                let body_preview = if body.len() > 200 { &body[..200] } else { &body };
                return Err(VenoreError::RagEmbeddingError(format!(
                    "OpenAI embedding API error ({}): {}", status, body_preview
                )));
            }

            let data: EmbeddingResponse = response.json().await
                .map_err(|e| VenoreError::RagEmbeddingError(format!("Failed to parse OpenAI embedding response: {}", e)))?;

            for item in data.data {
                all_embeddings.push(item.embedding);
            }
        }

        Ok(all_embeddings)
    }
}
