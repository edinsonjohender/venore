//! LLM validation utilities

use crate::{Result, VenoreError};

/// Validate temperature value (0.0 - 2.0)
pub fn validate_temperature(temp: f32) -> Result<()> {
    if !(0.0..=2.0).contains(&temp) {
        return Err(VenoreError::LlmInvalidRequest(format!(
            "Temperature must be between 0.0 and 2.0, got {}",
            temp
        )));
    }
    Ok(())
}

/// Validate max_tokens value (> 0)
pub fn validate_max_tokens(tokens: u32) -> Result<()> {
    if tokens == 0 {
        return Err(VenoreError::LlmInvalidRequest(
            "max_tokens must be greater than 0".to_string()
        ));
    }
    Ok(())
}

/// Validate model name (not empty)
pub fn validate_model_name(model: &str) -> Result<()> {
    if model.trim().is_empty() {
        return Err(VenoreError::LlmInvalidRequest(
            "Model name cannot be empty".to_string()
        ));
    }
    Ok(())
}
