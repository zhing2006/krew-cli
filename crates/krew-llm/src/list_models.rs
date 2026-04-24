//! List Models API — fetch available models from LLM providers.

use std::time::Duration;

use krew_config::ProviderType;
use serde::Deserialize;

use crate::LlmError;

/// Timeout for list models HTTP requests.
const LIST_MODELS_TIMEOUT: Duration = Duration::from_secs(15);

/// Information about a single model.
#[derive(Debug, Clone)]
pub struct ModelInfo {
    /// Model identifier (e.g. "claude-sonnet-4-6", "gpt-5.4").
    pub id: String,
}

/// Configuration for the list_models request.
pub struct ListModelsConfig {
    pub provider_type: ProviderType,
    pub base_url: Option<String>,
    pub api_key: String,
    pub vertex_project: Option<String>,
    pub vertex_location: Option<String>,
}

impl ListModelsConfig {
    /// Whether this is an OpenAI-compatible provider with a custom base URL
    /// (i.e. not the official OpenAI API).
    fn is_openai_compatible(&self) -> bool {
        self.provider_type == ProviderType::OpenAI
            && self
                .base_url
                .as_deref()
                .is_some_and(|u| !u.contains("api.openai.com"))
    }
}

/// Fetch available models from the given provider.
pub async fn list_models(config: &ListModelsConfig) -> Result<Vec<ModelInfo>, LlmError> {
    let mut models = if config.is_openai_compatible() {
        list_openai_compatible(config).await?
    } else {
        match config.provider_type {
            ProviderType::OpenAI => list_openai(config).await?,
            ProviderType::Anthropic => list_anthropic(config).await?,
            ProviderType::Google => {
                if config.vertex_project.is_some() && config.vertex_location.is_some() {
                    list_vertex(config).await?
                } else {
                    list_google_gemini(config).await?
                }
            }
        }
    };

    models.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(models)
}

/// Return hardcoded fallback models for a provider type.
pub fn fallback_models(provider_type: ProviderType) -> Vec<ModelInfo> {
    let ids = match provider_type {
        ProviderType::Anthropic => vec![
            "claude-opus-4-7",
            "claude-opus-4-6",
            "claude-sonnet-4-6",
            "claude-haiku-4-5-20251001",
        ],
        ProviderType::OpenAI => vec!["gpt-5.5", "gpt-5.4", "gpt-5.4-mini", "gpt-5.4-nano"],
        ProviderType::Google => vec!["gemini-3.1-pro-preview", "gemini-3.1-flash-lite-preview"],
    };
    ids.into_iter()
        .map(|id| ModelInfo { id: id.to_string() })
        .collect()
}

// ── OpenAI ──────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct OpenAiModelsResponse {
    data: Vec<OpenAiModel>,
}

#[derive(Deserialize)]
struct OpenAiModel {
    id: String,
}

async fn list_openai(config: &ListModelsConfig) -> Result<Vec<ModelInfo>, LlmError> {
    let base = config
        .base_url
        .as_deref()
        .unwrap_or("https://api.openai.com");
    let url = format!("{}/v1/models", base.trim_end_matches('/'));

    let client = reqwest::Client::builder()
        .timeout(LIST_MODELS_TIMEOUT)
        .build()
        .map_err(LlmError::Network)?;

    let resp = client
        .get(&url)
        .header("Authorization", format!("Bearer {}", config.api_key))
        .send()
        .await
        .map_err(LlmError::Network)?;

    if !resp.status().is_success() {
        return Err(LlmError::Api(format!(
            "OpenAI list models failed: {}",
            resp.status()
        )));
    }

    let body: OpenAiModelsResponse = resp.json().await.map_err(LlmError::Network)?;

    Ok(body
        .data
        .into_iter()
        .filter(|m| m.id.starts_with("gpt") || m.id.starts_with("o") || m.id.starts_with("chatgpt"))
        .map(|m| ModelInfo { id: m.id })
        .collect())
}

// ── OpenAI-Compatible ──────────────────────────────────────────────

/// Fetch models from an OpenAI-compatible provider without prefix filtering.
async fn list_openai_compatible(config: &ListModelsConfig) -> Result<Vec<ModelInfo>, LlmError> {
    let base = config
        .base_url
        .as_deref()
        .unwrap_or("https://api.openai.com");
    // Handle base URLs that already end with /v1.
    let url = if base.trim_end_matches('/').ends_with("/v1") {
        format!("{}/models", base.trim_end_matches('/'))
    } else {
        format!("{}/v1/models", base.trim_end_matches('/'))
    };

    let client = reqwest::Client::builder()
        .timeout(LIST_MODELS_TIMEOUT)
        .build()
        .map_err(LlmError::Network)?;

    let resp = client
        .get(&url)
        .header("Authorization", format!("Bearer {}", config.api_key))
        .send()
        .await
        .map_err(LlmError::Network)?;

    if !resp.status().is_success() {
        return Err(LlmError::Api(format!(
            "OpenAI-compatible list models failed: {}",
            resp.status()
        )));
    }

    let body: OpenAiModelsResponse = resp.json().await.map_err(LlmError::Network)?;

    Ok(body
        .data
        .into_iter()
        .map(|m| ModelInfo { id: m.id })
        .collect())
}

// ── Anthropic ───────────────────────────────────────────────────────

#[derive(Deserialize)]
struct AnthropicModelsResponse {
    data: Vec<AnthropicModel>,
}

#[derive(Deserialize)]
struct AnthropicModel {
    id: String,
}

async fn list_anthropic(config: &ListModelsConfig) -> Result<Vec<ModelInfo>, LlmError> {
    let base = config
        .base_url
        .as_deref()
        .unwrap_or("https://api.anthropic.com");
    let url = format!("{}/v1/models", base.trim_end_matches('/'));

    let client = reqwest::Client::builder()
        .timeout(LIST_MODELS_TIMEOUT)
        .build()
        .map_err(LlmError::Network)?;

    let resp = client
        .get(&url)
        .header("x-api-key", &config.api_key)
        .header("anthropic-version", "2023-06-01")
        .send()
        .await
        .map_err(LlmError::Network)?;

    if !resp.status().is_success() {
        return Err(LlmError::Api(format!(
            "Anthropic list models failed: {}",
            resp.status()
        )));
    }

    let body: AnthropicModelsResponse = resp.json().await.map_err(LlmError::Network)?;

    Ok(body
        .data
        .into_iter()
        .filter(|m| m.id.starts_with("claude-"))
        .map(|m| ModelInfo { id: m.id })
        .collect())
}

// ── Google Gemini API ───────────────────────────────────────────────

#[derive(Deserialize)]
struct GeminiModelsResponse {
    #[serde(default)]
    models: Vec<GeminiModel>,
}

#[derive(Deserialize)]
struct GeminiModel {
    name: String,
}

async fn list_google_gemini(config: &ListModelsConfig) -> Result<Vec<ModelInfo>, LlmError> {
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models?key={}&pageSize=1000",
        config.api_key
    );

    let client = reqwest::Client::builder()
        .timeout(LIST_MODELS_TIMEOUT)
        .build()
        .map_err(LlmError::Network)?;

    let resp = client.get(&url).send().await.map_err(LlmError::Network)?;

    if !resp.status().is_success() {
        return Err(LlmError::Api(format!(
            "Google list models failed: {}",
            resp.status()
        )));
    }

    let body: GeminiModelsResponse = resp.json().await.map_err(LlmError::Network)?;

    Ok(body
        .models
        .into_iter()
        .map(|m| {
            // Strip "models/" prefix.
            let id = m.name.strip_prefix("models/").unwrap_or(&m.name);
            id.to_string()
        })
        .filter(|id| id.starts_with("gemini-"))
        .map(|id| ModelInfo { id })
        .collect())
}

// ── Google Vertex AI ────────────────────────────────────────────────

#[derive(Deserialize)]
struct VertexModelsResponse {
    #[serde(default)]
    models: Vec<VertexModel>,
}

#[derive(Deserialize)]
struct VertexModel {
    name: String,
}

async fn list_vertex(config: &ListModelsConfig) -> Result<Vec<ModelInfo>, LlmError> {
    let project = config.vertex_project.as_deref().unwrap_or_default();
    let location = config.vertex_location.as_deref().unwrap_or_default();

    let url = format!(
        "https://{location}-aiplatform.googleapis.com/v1/projects/{project}/locations/{location}/publishers/google/models"
    );

    let client = reqwest::Client::builder()
        .timeout(LIST_MODELS_TIMEOUT)
        .build()
        .map_err(LlmError::Network)?;

    let resp = client
        .get(&url)
        .header("Authorization", format!("Bearer {}", config.api_key))
        .send()
        .await
        .map_err(LlmError::Network)?;

    if !resp.status().is_success() {
        return Err(LlmError::Api(format!(
            "Vertex AI list models failed: {}",
            resp.status()
        )));
    }

    let body: VertexModelsResponse = resp.json().await.map_err(LlmError::Network)?;

    Ok(body
        .models
        .into_iter()
        .map(|m| {
            // Strip "publishers/google/models/" prefix.
            let id = m
                .name
                .strip_prefix("publishers/google/models/")
                .unwrap_or(&m.name);
            id.to_string()
        })
        .filter(|id| id.starts_with("gemini-"))
        .map(|id| ModelInfo { id })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fallback_anthropic() {
        let models = fallback_models(ProviderType::Anthropic);
        assert_eq!(models.len(), 4);
        assert!(models.iter().any(|m| m.id == "claude-opus-4-7"));
        assert!(models.iter().any(|m| m.id == "claude-opus-4-6"));
        assert!(models.iter().any(|m| m.id == "claude-sonnet-4-6"));
        assert!(models.iter().any(|m| m.id == "claude-haiku-4-5-20251001"));
    }

    #[test]
    fn fallback_openai() {
        let models = fallback_models(ProviderType::OpenAI);
        assert_eq!(models.len(), 4);
        assert!(models.iter().any(|m| m.id == "gpt-5.5"));
        assert!(models.iter().any(|m| m.id == "gpt-5.4"));
        assert!(models.iter().any(|m| m.id == "gpt-5.4-mini"));
        assert!(models.iter().any(|m| m.id == "gpt-5.4-nano"));
    }

    #[test]
    fn fallback_google() {
        let models = fallback_models(ProviderType::Google);
        assert_eq!(models.len(), 2);
        assert!(models.iter().any(|m| m.id == "gemini-3.1-pro-preview"));
        assert!(
            models
                .iter()
                .any(|m| m.id == "gemini-3.1-flash-lite-preview")
        );
    }

    #[test]
    fn openai_filter_logic() {
        // Test that only gpt/o/chatgpt models pass the filter.
        let ids = vec![
            "gpt-5.4",
            "gpt-5.4-mini",
            "o1-preview",
            "chatgpt-4o-latest",
            "text-embedding-3-small",
            "dall-e-3",
            "tts-1",
            "whisper-1",
        ];

        let filtered: Vec<&str> = ids
            .into_iter()
            .filter(|id| id.starts_with("gpt") || id.starts_with("o") || id.starts_with("chatgpt"))
            .collect();

        assert_eq!(
            filtered,
            vec!["gpt-5.4", "gpt-5.4-mini", "o1-preview", "chatgpt-4o-latest"]
        );
    }

    #[test]
    fn anthropic_filter_logic() {
        let ids = vec!["claude-opus-4-6", "claude-sonnet-4-6", "some-other-model"];
        let filtered: Vec<&str> = ids
            .into_iter()
            .filter(|id| id.starts_with("claude-"))
            .collect();
        assert_eq!(filtered, vec!["claude-opus-4-6", "claude-sonnet-4-6"]);
    }

    #[test]
    fn google_model_id_extraction() {
        let name = "models/gemini-3.1-pro-preview";
        let id = name.strip_prefix("models/").unwrap_or(name);
        assert_eq!(id, "gemini-3.1-pro-preview");
    }

    #[test]
    fn vertex_model_id_extraction() {
        let name = "publishers/google/models/gemini-3.1-pro-preview";
        let id = name
            .strip_prefix("publishers/google/models/")
            .unwrap_or(name);
        assert_eq!(id, "gemini-3.1-pro-preview");
    }

    #[test]
    fn vertex_url_construction() {
        let project = "my-project";
        let location = "us-central1";
        let url = format!(
            "https://{location}-aiplatform.googleapis.com/v1/projects/{project}/locations/{location}/publishers/google/models"
        );
        assert_eq!(
            url,
            "https://us-central1-aiplatform.googleapis.com/v1/projects/my-project/locations/us-central1/publishers/google/models"
        );
    }

    #[test]
    fn sorting() {
        let mut models = [
            ModelInfo {
                id: "claude-sonnet-4-6".into(),
            },
            ModelInfo {
                id: "claude-haiku-4-5-20251001".into(),
            },
            ModelInfo {
                id: "claude-opus-4-6".into(),
            },
        ];
        models.sort_by(|a, b| a.id.cmp(&b.id));
        assert_eq!(models[0].id, "claude-haiku-4-5-20251001");
        assert_eq!(models[1].id, "claude-opus-4-6");
        assert_eq!(models[2].id, "claude-sonnet-4-6");
    }
}
