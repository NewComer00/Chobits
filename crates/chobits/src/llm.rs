use crate::config::LlmConfig;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct LlmResponse {
    pub text: String,
    pub expression: String,
}

pub enum Backend {
    Ollama {
        url: String,
        model: String,
        max_tokens: u32,
    },
    OpenAiCompatible {
        url: String,
        model: String,
        api_key: String,
        max_tokens: u32,
    },
}

impl Backend {
    pub fn from_config(cfg: &LlmConfig) -> Self {
        let api_key = cfg.api_key.clone();
        let max_tokens = cfg.max_tokens;
        match cfg.backend.as_str() {
            "ollama" => Backend::Ollama {
                url: cfg.url.clone(),
                model: cfg.model.clone(),
                max_tokens,
            },
            _ => Backend::OpenAiCompatible {
                url: cfg.url.clone(),
                model: cfg.model.clone(),
                api_key,
                max_tokens,
            },
        }
    }

    pub fn query(&self, prompt: &str) -> Option<String> {
        match self {
            Backend::Ollama {
                url,
                model,
                max_tokens,
            } => query_ollama(url, model, *max_tokens, prompt),
            Backend::OpenAiCompatible {
                url,
                model,
                api_key,
                max_tokens,
            } => query_openai(url, model, api_key, *max_tokens, prompt),
        }
    }
}

/// Build the system prompt using alias labels only.
pub fn build_system_prompt(
    name: &str,
    persona_description: &str,
    aliases: &[String],
    default_alias: &str,
) -> String {
    let list = aliases.join(", ");
    format!(
        "\
You are {name}.\n\
{persona_description}\n\
\n\
OUTPUT FORMAT — reply with exactly one JSON object, no other text:\n\
{{\"text\": \"<your reaction, 1-2 sentences>\", \"expression\": \"<alias>\"}}\n\
\n\
EXPRESSION ALIASES — the \"expression\" value MUST be exactly one of these (copy verbatim):\n\
{list}\n\
Default resting look: {default_alias}. Pick the alias that best matches your reaction."
    )
}

fn query_ollama(url: &str, model: &str, max_tokens: u32, prompt: &str) -> Option<String> {
    let body = serde_json::json!({
        "model": model,
        "prompt": prompt,
        "stream": false,
        "options":{"num_predict": max_tokens},
    });

    let resp = ureq::post(&format!("{}/api/generate", url))
        .set("Content-Type", "application/json")
        .send_string(&body.to_string());

    if resp.synthetic_error().is_some() {
        eprintln!("[llm/ollama] request failed — status {}", resp.status());
        return None;
    }

    let raw = resp.into_string().ok()?;
    let json: serde_json::Value = serde_json::from_str(&raw).ok()?;
    json.get("response")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

fn query_openai(
    url: &str,
    model: &str,
    api_key: &str,
    max_tokens: u32,
    prompt: &str,
) -> Option<String> {
    let body = serde_json::json!({
        "model": model,
        "messages": [
            {"role": "user", "content": prompt}
        ],
        "temperature": 0.7,
        "max_tokens": max_tokens,
        "response_format": {"type": "json_object"},
    });

    let mut binding = ureq::post(&format!("{}/v1/chat/completions", url));
    let mut req = binding.set("Content-Type", "application/json");

    if !api_key.is_empty() {
        req = req.set("Authorization", &format!("Bearer {}", api_key));
    }

    let resp = req.send_string(&body.to_string());

    if resp.synthetic_error().is_some() {
        eprintln!("[llm/openai] request failed — status {}", resp.status());
        return None;
    }

    let raw = resp.into_string().ok()?;
    let json: serde_json::Value = serde_json::from_str(&raw).ok()?;
    json.get("choices")
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.first())
        .and_then(|c| c.get("message"))
        .and_then(|m| m.get("content"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

pub fn parse_response(raw: &str, fallback_alias: &str) -> Option<LlmResponse> {
    match jsonrepair_rs::jsonrepair_parse::<LlmResponse>(raw) {
        Ok(mut response) => {
            if response.expression.is_empty() {
                response.expression = fallback_alias.to_string();
            }
            Some(response)
        }
        Err(_) => {
            eprintln!("[llm] parse_response: jsonrepair could not recover a valid LlmResponse\n  raw: {raw:?}");
            Some(LlmResponse {
                text: raw.lines().take(2).collect::<Vec<_>>().join(" "),
                expression: fallback_alias.to_string(),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::LlmConfig;

    fn sample_llm_config(backend: &str) -> LlmConfig {
        LlmConfig {
            backend: backend.into(),
            url: "http://127.0.0.1:11434".into(),
            model: "test-model".into(),
            max_tokens: 128,
            api_key: "secret".into(),
        }
    }

    #[test]
    fn build_system_prompt_lists_aliases_only() {
        let prompt = build_system_prompt(
            "Chi",
            "Warm and curious.",
            &["idle".into(), "wave".into(), "bounce".into()],
            "idle",
        );
        assert!(prompt.contains("idle, wave, bounce"));
        assert!(prompt.contains("Default resting look: idle"));
        assert!(!prompt.contains("idle_0"));
        assert!(!prompt.contains("Idle #0"));
    }

    #[test]
    fn parse_response_uses_alias_fallback() {
        let raw = r#"{"text":"only text"}"#;
        let parsed = parse_response(raw, "idle").expect("fallback");
        assert_eq!(parsed.expression, "idle");
    }

    #[test]
    fn parse_response_accepts_clean_json() {
        let raw = r#"{"text":"nice work!","expression":"happy"}"#;
        let parsed = parse_response(raw, "idle").expect("parsed");
        assert_eq!(parsed.text, "nice work!");
        assert_eq!(parsed.expression, "happy");
    }

    #[test]
    fn parse_response_repairs_markdown_fenced_json() {
        let raw = "```json\n{\"text\":\"hey\",\"expression\":\"blink\"}\n```";
        let parsed = parse_response(raw, "idle").expect("parsed");
        assert_eq!(parsed.text, "hey");
        assert_eq!(parsed.expression, "blink");
    }

    #[test]
    fn from_config_selects_ollama_backend() {
        let cfg = sample_llm_config("ollama");
        match Backend::from_config(&cfg) {
            Backend::Ollama {
                model, max_tokens, ..
            } => {
                assert_eq!(model, "test-model");
                assert_eq!(max_tokens, 128);
            }
            Backend::OpenAiCompatible { .. } => panic!("expected ollama backend"),
        }
    }
}
