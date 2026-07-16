//! Live meeting copilot: given the recent transcript and the user's resume
//! context, suggest an answer to the most recent question in real time.
//!
//! Reuses the summary LLM client. Provider is resolved by priority
//! (Gemini → Groq → the user's currently-configured summary provider) so the
//! copilot keeps working even if the configured provider is unusable.

use crate::database::repositories::setting::SettingsRepository;
use crate::state::AppState;
use crate::summary::llm_client::{generate_summary, LLMProvider};
use tauri::Runtime;

/// Preferred model-family order within 9Router. The first available 9Router
/// model whose id contains one of these substrings wins.
const NINEROUTER_MODEL_PRIORITY: &[&str] =
    &["gemini", "groq", "openrouter", "deepseek"];

/// A resolved provider ready to call.
struct ResolvedProvider {
    provider: LLMProvider,
    model: String,
    api_key: String,
}

/// Resolve the provider + model for a copilot call.
///
/// All the user's preferred models (gemini/groq/openrouter/deepseek) are
/// served through 9Router, so we stay on 9Router and pick the best *available*
/// model by the priority order — which also avoids the stale-model 404 the user
/// hit, since we only choose from models 9Router currently reports.
async fn resolve_provider(state: &AppState, prompt: &str) -> Result<ResolvedProvider, String> {
    let pool = state.db_manager.pool();

    // 9Router key is optional (only when the router runs with REQUIRE_API_KEY).
    let api_key = SettingsRepository::get_api_key(pool, "9router")
        .await
        .ok()
        .flatten()
        .unwrap_or_default();

    let model = pick_ninerouter_model(api_key.clone(), prompt).await?;

    Ok(ResolvedProvider {
        provider: LLMProvider::NineRouter,
        model,
        api_key,
    })
}

/// Pick a 9Router model: filter to the preferred families
/// ([`NINEROUTER_MODEL_PRIORITY`]) then let the auto-model heuristic choose an
/// easy vs strong variant based on how hard the current question is.
async fn pick_ninerouter_model(api_key: String, prompt: &str) -> Result<String, String> {
    let key = if api_key.trim().is_empty() {
        None
    } else {
        Some(api_key)
    };
    let models = crate::ninerouter::get_ninerouter_models(None, key).await?;
    if models.is_empty() {
        return Err("9Router reported no available models.".to_string());
    }

    // Prefer the first available family in priority order; collect all of its
    // variants so the difficulty heuristic can pick easy vs strong within it.
    let mut candidates: Vec<String> = Vec::new();
    for family in NINEROUTER_MODEL_PRIORITY {
        candidates = models
            .iter()
            .filter(|m| m.id.to_lowercase().contains(family))
            .map(|m| m.id.clone())
            .collect();
        if !candidates.is_empty() {
            break;
        }
    }
    if candidates.is_empty() {
        candidates = models.iter().map(|m| m.id.clone()).collect();
    }

    Ok(crate::summary::auto_model::resolve_model(
        crate::summary::auto_model::AUTO_MODEL,
        &LLMProvider::NineRouter,
        prompt,
        &candidates,
    ))
}

/// Generate a suggested answer for the most recent question in the transcript.
///
/// `transcript` is the recent meeting transcript (most recent last).
/// Returns the suggested answer text.
#[tauri::command]
pub async fn copilot_suggest_answer<R: Runtime>(
    _app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    transcript: String,
) -> Result<String, String> {
    if transcript.trim().is_empty() {
        return Err("Transcript is empty.".to_string());
    }

    let resolved = resolve_provider(&state, &transcript).await?;

    // Load resume context if present.
    let resume = SettingsRepository::get_resume_context(state.db_manager.pool())
        .await
        .ok()
        .flatten()
        .map(|(content, _)| content)
        .filter(|c| !c.trim().is_empty());

    let system_prompt = build_system_prompt(resume.as_deref());
    let user_prompt = format!(
        "Recent meeting transcript (most recent line last):\n\n{}\n\n\
         Based on the latest question or prompt directed at me, give me a concise, \
         confident answer I can say out loud. If no direct question was asked, reply with an \
         empty response.",
        transcript.trim()
    );

    let client = reqwest::Client::new();
    generate_summary(
        &client,
        &resolved.provider,
        &resolved.model,
        &resolved.api_key,
        &system_prompt,
        &user_prompt,
        None,
        None,
        Some(400),
        Some(0.4),
        None,
        None,
        None,
    )
    .await
}

fn build_system_prompt(resume: Option<&str>) -> String {
    let mut prompt = String::from(
        "You are a real-time interview/meeting copilot for the user. You listen to the live \
         transcript and, when someone asks the user a question, you draft a strong first-person \
         answer the user can speak immediately. Be concise (2-4 sentences), specific, and \
         confident. Answer as the user (\"I\"). Do not add preamble like 'You could say'.",
    );
    if let Some(r) = resume {
        prompt.push_str(
            "\n\nUse the following background about the user (from their resume) to ground \
             answers in their real experience:\n",
        );
        prompt.push_str(r.trim());
    }
    prompt
}
