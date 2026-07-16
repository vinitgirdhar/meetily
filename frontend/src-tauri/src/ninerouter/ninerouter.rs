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

/// Rank a 9Router model id by a rough capability heuristic.
///
/// Higher is better. Purely name-based (no live benchmarking) so it is cheap and
/// offline. Prefers larger parameter counts and known strong model families,
/// penalizes obviously small/fast/preview variants.
// ponytail: name-based heuristic; swap for a real capability lookup if 9Router ever exposes one
fn score_model_id(id: &str) -> i64 {
    let lower = id.to_lowercase();
    let mut score: i64 = 0;

    // Parameter size: pick the largest "<n>b" token in the name (e.g. llama-3.3-70b -> 70)
    let bytes = lower.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i].is_ascii_digit() {
            let start = i;
            while i < bytes.len() && bytes[i].is_ascii_digit() {
                i += 1;
            }
            if i < bytes.len() && bytes[i] == b'b' {
                if let Ok(n) = lower[start..i].parse::<i64>() {
                    // Weight parameter count heavily; cap so a 405b doesn't dwarf everything
                    score = score.max(n.min(500) * 10);
                }
            }
        } else {
            i += 1;
        }
    }

    // Known strong families
    for kw in ["gpt-4", "claude", "opus", "sonnet", "deepseek", "qwen", "llama-3.3", "mixtral"] {
        if lower.contains(kw) {
            score += 40;
        }
    }

    // Penalize small/fast/preview variants
    for kw in ["mini", "nano", "tiny", "small", "1b", "3b", "haiku", "instant", "preview"] {
        if lower.contains(kw) {
            score -= 30;
        }
    }

    score
}

/// Auto-select the "best" available 9Router model using [`score_model_id`].
///
/// Returns the highest-scoring model id, or an error if none are available.
#[command]
pub async fn ninerouter_auto_select_model(
    endpoint: Option<String>,
    api_key: Option<String>,
) -> Result<String, String> {
    let models = get_ninerouter_models(endpoint, api_key).await?;
    models
        .into_iter()
        .max_by_key(|m| score_model_id(&m.id))
        .map(|m| m.id)
        .ok_or_else(|| "No models available from 9Router".to_string())
}

/// Test connectivity to a 9Router instance.
///
/// Verifies the /v1/models endpoint is reachable and returns at least one model.
#[command]
pub async fn test_ninerouter_connection(
    endpoint: Option<String>,
    api_key: Option<String>,
) -> Result<serde_json::Value, String> {
    let models = get_ninerouter_models(endpoint, api_key).await?;
    if models.is_empty() {
        return Err("Connected to 9Router but no models are available.".to_string());
    }
    Ok(serde_json::json!({
        "status": "success",
        "message": format!("Connection successful — {} models available", models.len()),
        "model_count": models.len(),
    }))
}

#[cfg(test)]
mod tests {
    use super::score_model_id;

    #[test]
    fn larger_model_beats_smaller() {
        assert!(score_model_id("llama-3.3-70b-versatile") > score_model_id("llama-3.2-1b"));
    }

    #[test]
    fn mini_variant_penalized() {
        assert!(score_model_id("gpt-4o") > score_model_id("gpt-4o-mini"));
    }

    #[test]
    fn empty_list_has_no_panic() {
        // sanity: scoring never panics on odd names
        let _ = score_model_id("");
        let _ = score_model_id("weird::name-b");
    }
}
