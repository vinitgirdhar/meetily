//! Auto model selection: pick an easy or strong model per request based on a
//! fast, local task-difficulty heuristic. No extra LLM round-trip.
//!
//! Used by both summary generation and the live copilot so the user never has
//! to manually switch models per task.

use crate::summary::llm_client::LLMProvider;

/// The sentinel model value meaning "let the app choose per request".
pub const AUTO_MODEL: &str = "auto";

/// Difficulty tier for a request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Difficulty {
    Easy,
    Hard,
}

/// Keywords that signal a genuinely hard/analytical task.
const HARD_KEYWORDS: &[&str] = &[
    "analyze", "analyse", "design", "architect", "debug", "root cause", "trade-off",
    "tradeoff", "algorithm", "optimize", "optimise", "prove", "derive", "reason",
    "compare", "evaluate", "strategy", "refactor", "complex", "explain why",
    "step by step", "step-by-step", "in depth", "in-depth", "critique", "implications",
];

/// Keywords that signal an easy/mechanical task.
const EASY_KEYWORDS: &[&str] = &[
    "summarize", "summarise", "list", "tl;dr", "tldr", "rephrase", "reword",
    "translate", "format", "extract", "title", "bullet", "shorten", "yes or no",
];

/// Classify request difficulty from the prompt text using a fast heuristic.
///
/// Signals: explicit hard/easy keywords, then length as a tiebreaker.
pub fn classify_difficulty(text: &str) -> Difficulty {
    let lower = text.to_lowercase();

    let hard_hits = HARD_KEYWORDS.iter().filter(|k| lower.contains(**k)).count();
    let easy_hits = EASY_KEYWORDS.iter().filter(|k| lower.contains(**k)).count();

    if hard_hits > easy_hits {
        return Difficulty::Hard;
    }
    if easy_hits > hard_hits {
        return Difficulty::Easy;
    }

    // Tie / no keywords: use length. Long transcripts or prompts tend to need
    // the stronger model to stay coherent.
    let word_count = text.split_whitespace().count();
    if word_count > 400 {
        Difficulty::Hard
    } else {
        Difficulty::Easy
    }
}

/// Built-in easy/strong model tiers per provider.
///
/// Returns (easy_model, strong_model). For providers whose model ids are
/// dynamic (Ollama, 9Router, OpenRouter, CustomOpenAI, BuiltInAI) there is no
/// static tier — callers should resolve those from the live model list.
fn static_tiers(provider: &LLMProvider) -> Option<(&'static str, &'static str)> {
    match provider {
        LLMProvider::Gemini => Some(("gemini-2.5-flash", "gemini-2.5-pro")),
        LLMProvider::Groq => Some(("llama-3.1-8b-instant", "llama-3.3-70b-versatile")),
        LLMProvider::OpenAI => Some(("gpt-4o-mini", "gpt-4o")),
        LLMProvider::Claude => Some(("claude-haiku-4-5-20251001", "claude-sonnet-4-5-20250929")),
        _ => None,
    }
}

/// Resolve the concrete model to use for a request.
///
/// * `configured_model` - what the user selected (may be [`AUTO_MODEL`]).
/// * `provider` - the active provider.
/// * `prompt` - the request text used to judge difficulty.
/// * `available` - live model ids (for dynamic providers like 9Router/Ollama).
///
/// If the model isn't "auto", it's returned as-is. Otherwise difficulty is
/// classified and mapped to a tier.
pub fn resolve_model(
    configured_model: &str,
    provider: &LLMProvider,
    prompt: &str,
    available: &[String],
) -> String {
    if !configured_model.eq_ignore_ascii_case(AUTO_MODEL) {
        return configured_model.to_string();
    }

    let difficulty = classify_difficulty(prompt);

    if let Some((easy, strong)) = static_tiers(provider) {
        return match difficulty {
            Difficulty::Easy => easy.to_string(),
            Difficulty::Hard => strong.to_string(),
        };
    }

    // Dynamic providers: pick from the live list by name heuristic.
    pick_dynamic(difficulty, available)
}

/// Substrings hinting a small/fast model (used for the "easy" tier on dynamic
/// providers) and a large/strong model (for the "hard" tier).
const SMALL_HINTS: &[&str] = &["mini", "flash", "8b", "7b", "small", "haiku", "lite", "instant", "nano"];
const LARGE_HINTS: &[&str] = &["70b", "72b", "pro", "large", "opus", "ultra", "405b", "sonnet", "deepseek"];

fn pick_dynamic(difficulty: Difficulty, available: &[String]) -> String {
    if available.is_empty() {
        return AUTO_MODEL.to_string(); // caller will surface a "no models" error
    }
    let hints: &[&str] = match difficulty {
        Difficulty::Easy => SMALL_HINTS,
        Difficulty::Hard => LARGE_HINTS,
    };
    for m in available {
        let lower = m.to_lowercase();
        if hints.iter().any(|h| lower.contains(h)) {
            return m.clone();
        }
    }
    // No hint matched: first model is a safe default.
    available[0].clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hard_keyword_wins() {
        assert_eq!(classify_difficulty("Analyze the trade-offs of this design"), Difficulty::Hard);
    }

    #[test]
    fn easy_keyword_wins() {
        assert_eq!(classify_difficulty("summarize this in one line"), Difficulty::Easy);
    }

    #[test]
    fn static_tier_maps_difficulty() {
        let easy = resolve_model(AUTO_MODEL, &LLMProvider::Gemini, "list the names", &[]);
        let hard = resolve_model(AUTO_MODEL, &LLMProvider::Gemini, "analyze and design a system", &[]);
        assert_eq!(easy, "gemini-2.5-flash");
        assert_eq!(hard, "gemini-2.5-pro");
    }

    #[test]
    fn explicit_model_passes_through() {
        let m = resolve_model("gpt-4o", &LLMProvider::OpenAI, "anything", &[]);
        assert_eq!(m, "gpt-4o");
    }

    #[test]
    fn dynamic_picks_by_hint() {
        let models = vec!["router/gemini-flash".to_string(), "router/gemini-2.5-pro".to_string()];
        let hard = resolve_model(AUTO_MODEL, &LLMProvider::NineRouter, "analyze design tradeoffs", &models);
        assert!(hard.contains("pro"));
    }
}
