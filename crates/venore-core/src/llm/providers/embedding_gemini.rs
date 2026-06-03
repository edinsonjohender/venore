//! Gemini Embedding Provider
//!
//! Implements EmbeddingProvider for Google's Gemini embedding models.
//! Default model: text-embedding-004 (768 dimensions).
//!
//! ## API Documentation
//! - Endpoint: POST https://generativelanguage.googleapis.com/v1beta/models/{model}:batchEmbedContents
//! - Docs: https://ai.google.dev/docs/embeddings

use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::traits::EmbeddingProvider;
use crate::{Result, VenoreError};

const API_BASE: &str = "https://generativelanguage.googleapis.com/v1beta";
const DEFAULT_DIMENSIONS: u32 = 768;
const MAX_BATCH_SIZE: usize = 100;

// ============================================================================
// API TYPES
// ============================================================================

#[derive(Serialize)]
struct BatchEmbedRequest {
    requests: Vec<EmbedContentRequest>,
}

#[derive(Serialize)]
struct EmbedContentRequest {
    model: String,
    content: GeminiContent,
}

#[derive(Serialize)]
struct GeminiContent {
    parts: Vec<GeminiPart>,
}

#[derive(Serialize)]
struct GeminiPart {
    text: String,
}

#[derive(Deserialize)]
struct BatchEmbedResponse {
    embeddings: Vec<GeminiEmbedding>,
}

#[derive(Deserialize)]
struct GeminiEmbedding {
    values: Vec<f32>,
}

// ============================================================================
// PROVIDER
// ============================================================================

pub struct GeminiEmbeddingProvider {
    model: String,
    client: Client,
}

impl GeminiEmbeddingProvider {
    pub fn new(model: String) -> Self {
        Self {
            model,
            client: Client::new(),
        }
    }
}

#[async_trait]
impl EmbeddingProvider for GeminiEmbeddingProvider {
    fn provider_name(&self) -> &str {
        "gemini"
    }

    fn dimensions(&self) -> u32 {
        DEFAULT_DIMENSIONS
    }

    fn model(&self) -> &str {
        &self.model
    }

    async fn embed_batch(&self, api_key: &str, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        let mut all_embeddings = Vec::with_capacity(texts.len());
        let model_path = format!("models/{}", self.model);

        for batch in texts.chunks(MAX_BATCH_SIZE) {
            let requests: Vec<EmbedContentRequest> = batch.iter()
                .map(|text| EmbedContentRequest {
                    model: model_path.clone(),
                    content: GeminiContent {
                        parts: vec![GeminiPart { text: text.clone() }],
                    },
                })
                .collect();

            let url = format!(
                "{}/models/{}:batchEmbedContents?key={}",
                API_BASE, self.model, api_key
            );

            let body = BatchEmbedRequest { requests };

            let response = self.client
                .post(&url)
                .header("Content-Type", "application/json")
                .json(&body)
                .send()
                .await
                .map_err(|e| VenoreError::RagEmbeddingError(format!("Gemini embedding request failed: {}", e)))?;

            if !response.status().is_success() {
                let status = response.status().as_u16();
                let body = response.text().await.unwrap_or_default();
                let body_preview = if body.len() > 200 { &body[..200] } else { &body };
                return Err(VenoreError::RagEmbeddingError(format!(
                    "Gemini embedding API error ({}): {}", status, body_preview
                )));
            }

            let data: BatchEmbedResponse = response.json().await
                .map_err(|e| VenoreError::RagEmbeddingError(format!("Failed to parse Gemini embedding response: {}", e)))?;

            for emb in data.embeddings {
                all_embeddings.push(emb.values);
            }
        }

        Ok(all_embeddings)
    }
}
