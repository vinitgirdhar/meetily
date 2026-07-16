use serde::{Deserialize, Serialize};
use std::time::Duration;
use tauri::command;

/// Default base URL for a local 9Router instance
pub const DEFAULT_NINEROUTER_ENDPOINT: &str = "http://localhost:20128";

/// 9Router model information returned to frontend
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NineRouterModel {
    pub id: String,
}

/// API response model from 9Router (OpenAI-compatible /v1/models format)
#[derive(Debug, Deserialize)]
struct NineRouterApiModel {
    id: String,
}

/// API response wrapper from 9Router
#[derive(Debug, Deserialize)]
struct NineRouterApiResponse {
    data: Vec<NineRouterApiModel>,
}

/// Fetch available models from a 9Router instance
///
/// # Arguments
/// * `endpoint` - Optional custom 9Router base URL (defaults to http://localhost:20128)
/// * `api_key` - Optional API key (only needed when 9Router runs with REQUIRE_API_KEY)
///
/// # Returns
/// Vector of available models, or an error message
#[command]
pub async fn get_ninerouter_models(
    endpoint: Option<String>,
    api_key: Option<String>,
) -> Result<Vec<NineRouterModel>, String> {
    let base = endpoint
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(DEFAULT_NINEROUTER_ENDPOINT)
        .trim_end_matches('/')
        .to_string();

    let url = format!("{}/v1/models", base);
    log::info!("Fetching 9Router models from {}", url);

    let client = reqwest::Client::new();
    let mut request = client.get(&url).timeout(Duration::from_secs(10));

    if let Some(key) = api_key.as_deref().map(str::trim).filter(|k| !k.is_empty()) {
        request = request.header("Authorization", format!("Bearer {}", key));
    }

    let response = request.send().await.map_err(|e| {
        format!(
            "Failed to connect to 9Router at {}: {}. Make sure 9Router is running.",
            base, e
        )
    })?;

    if !response.status().is_success() {
        return Err(format!(
            "9Router request failed with status: {}",
            response.status()
        ));
    }

    let api_response: NineRouterApiResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse 9Router response: {}", e))?;

    let models = api_response
        .data
        .into_iter()
        .map(|m| NineRouterModel { id: m.id })
        .collect();

    Ok(models)
}
