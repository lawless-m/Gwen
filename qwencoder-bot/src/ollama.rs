//! Ollama API client for QwenCoder Bot
//!
//! Provides async interface to Ollama's REST API for LLM inference.

use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{debug, error, info, warn};

/// Ollama API client
#[derive(Clone)]
pub struct OllamaClient {
    client: Client,
    endpoint: String,
    timeout: Duration,
}

/// Request body for Ollama's generate endpoint
#[derive(Debug, Serialize)]
pub struct GenerateRequest {
    pub model: String,
    pub prompt: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,
    pub stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<GenerateOptions>,
}

/// Model options for generation
#[derive(Debug, Serialize, Default)]
pub struct GenerateOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub num_predict: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop: Option<Vec<String>>,
}

/// Response from Ollama's generate endpoint
#[derive(Debug, Deserialize)]
pub struct GenerateResponse {
    pub model: String,
    pub response: String,
    pub done: bool,
    #[serde(default)]
    pub context: Option<Vec<i64>>,
    #[serde(default)]
    pub total_duration: Option<u64>,
    #[serde(default)]
    pub load_duration: Option<u64>,
    #[serde(default)]
    pub prompt_eval_count: Option<u32>,
    #[serde(default)]
    pub prompt_eval_duration: Option<u64>,
    #[serde(default)]
    pub eval_count: Option<u32>,
    #[serde(default)]
    pub eval_duration: Option<u64>,
}

/// Response from Ollama's list endpoint
#[derive(Debug, Deserialize)]
pub struct ListModelsResponse {
    pub models: Vec<ModelInfo>,
}

/// Information about a model
#[derive(Debug, Deserialize)]
pub struct ModelInfo {
    pub name: String,
    pub modified_at: String,
    pub size: u64,
    pub digest: String,
}

/// Error types for Ollama operations
#[derive(Debug, thiserror::Error)]
pub enum OllamaError {
    #[error("Request failed: {0}")]
    RequestError(#[from] reqwest::Error),

    #[error("Model not found: {0}")]
    ModelNotFound(String),

    #[error("Invalid response from Ollama: {0}")]
    InvalidResponse(String),

    #[error("Timeout waiting for response")]
    Timeout,

    #[error("Failed to parse JSON response: {0}")]
    ParseError(String),
}

impl OllamaClient {
    /// Create a new Ollama client
    pub fn new(endpoint: &str, timeout_secs: u64) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(timeout_secs))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            endpoint: endpoint.trim_end_matches('/').to_string(),
            timeout: Duration::from_secs(timeout_secs),
        }
    }

    /// Check if Ollama server is reachable
    pub async fn health_check(&self) -> Result<bool> {
        let url = format!("{}/api/tags", self.endpoint);

        match self.client.get(&url).send().await {
            Ok(response) => Ok(response.status().is_success()),
            Err(e) => {
                warn!("Ollama health check failed: {}", e);
                Ok(false)
            }
        }
    }

    /// List available models
    pub async fn list_models(&self) -> Result<Vec<ModelInfo>> {
        let url = format!("{}/api/tags", self.endpoint);

        let response = self.client
            .get(&url)
            .send()
            .await
            .context("Failed to connect to Ollama")?;

        if !response.status().is_success() {
            anyhow::bail!("Failed to list models: HTTP {}", response.status());
        }

        let list_response: ListModelsResponse = response
            .json()
            .await
            .context("Failed to parse model list response")?;

        Ok(list_response.models)
    }

    /// Check if a specific model is available
    pub async fn has_model(&self, model: &str) -> Result<bool> {
        let models = self.list_models().await?;
        Ok(models.iter().any(|m| m.name == model))
    }

    /// Generate a completion from the model
    pub async fn generate(
        &self,
        model: &str,
        prompt: &str,
        system: Option<&str>,
        options: Option<GenerateOptions>,
    ) -> Result<GenerateResponse> {
        let url = format!("{}/api/generate", self.endpoint);

        let request = GenerateRequest {
            model: model.to_string(),
            prompt: prompt.to_string(),
            system: system.map(String::from),
            stream: false,
            options,
        };

        debug!("Sending generate request to {}", model);
        debug!("Prompt length: {} chars", prompt.len());

        let response = self.client
            .post(&url)
            .json(&request)
            .send()
            .await
            .context("Failed to send generate request")?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            error!("Ollama generate failed: HTTP {} - {}", status, error_text);

            if error_text.contains("model") && error_text.contains("not found") {
                return Err(OllamaError::ModelNotFound(model.to_string()).into());
            }

            anyhow::bail!("Generate request failed: HTTP {} - {}", status, error_text);
        }

        let generate_response: GenerateResponse = response
            .json()
            .await
            .context("Failed to parse generate response")?;

        if let Some(duration) = generate_response.total_duration {
            let duration_secs = duration as f64 / 1_000_000_000.0;
            info!(
                "Generation completed in {:.2}s ({} tokens)",
                duration_secs,
                generate_response.eval_count.unwrap_or(0)
            );
        }

        Ok(generate_response)
    }

    /// Generate with retry logic
    pub async fn generate_with_retry(
        &self,
        model: &str,
        prompt: &str,
        system: Option<&str>,
        options: Option<GenerateOptions>,
        max_retries: u32,
    ) -> Result<GenerateResponse> {
        let mut last_error = None;
        let mut delay = Duration::from_secs(2);

        for attempt in 0..=max_retries {
            if attempt > 0 {
                info!("Retry attempt {} after {:?}", attempt, delay);
                tokio::time::sleep(delay).await;
                delay *= 2; // Exponential backoff
            }

            match self.generate(model, prompt, system, options.clone()).await {
                Ok(response) => return Ok(response),
                Err(e) => {
                    warn!("Generate attempt {} failed: {}", attempt + 1, e);
                    last_error = Some(e);
                }
            }
        }

        Err(last_error.unwrap_or_else(|| anyhow::anyhow!("All retries exhausted")))
    }

    /// Wait for model to load (useful after KEEP_ALIVE expiry)
    pub async fn warm_up_model(&self, model: &str) -> Result<()> {
        info!("Warming up model: {}", model);

        let options = GenerateOptions {
            num_predict: Some(1),
            ..Default::default()
        };

        self.generate(model, "hello", None, Some(options)).await?;

        info!("Model {} is now loaded", model);
        Ok(())
    }

    /// Extract JSON from model response (handles markdown code blocks)
    pub fn extract_json(response: &str) -> Option<&str> {
        // Try to find JSON in code blocks first
        if let Some(start) = response.find("```json") {
            let json_start = start + 7;
            if let Some(end) = response[json_start..].find("```") {
                return Some(response[json_start..json_start + end].trim());
            }
        }

        // Try generic code blocks
        if let Some(start) = response.find("```") {
            let block_start = start + 3;
            // Skip language identifier on same line
            let content_start = response[block_start..]
                .find('\n')
                .map(|i| block_start + i + 1)
                .unwrap_or(block_start);

            if let Some(end) = response[content_start..].find("```") {
                return Some(response[content_start..content_start + end].trim());
            }
        }

        // Try to find raw JSON object
        if let Some(start) = response.find('{') {
            // Find matching closing brace
            let mut depth = 0;
            let mut end = start;

            for (i, c) in response[start..].char_indices() {
                match c {
                    '{' => depth += 1,
                    '}' => {
                        depth -= 1;
                        if depth == 0 {
                            end = start + i + 1;
                            break;
                        }
                    }
                    _ => {}
                }
            }

            if depth == 0 && end > start {
                return Some(&response[start..end]);
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_json_from_code_block() {
        let response = r#"Here is the assessment:

```json
{"decision": "worth-attempting", "confidence": 85}
```

Let me know if you need more details."#;

        let json = OllamaClient::extract_json(response).unwrap();
        assert!(json.contains("worth-attempting"));
    }

    #[test]
    fn test_extract_json_raw() {
        let response = r#"{"decision": "trivial", "confidence": 95}"#;

        let json = OllamaClient::extract_json(response).unwrap();
        assert!(json.contains("trivial"));
    }

    #[test]
    fn test_extract_json_nested() {
        let response = r#"{"outer": {"inner": {"value": 1}}, "end": true}"#;

        let json = OllamaClient::extract_json(response).unwrap();
        assert!(json.contains("inner"));
        assert!(json.ends_with('}'));
    }
}
