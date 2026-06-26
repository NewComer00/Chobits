use crate::config::LlmConfig;
use serde::Deserialize;
use std::path::Path;

// ── parsed LLM response (always the same shape) ──────────

#[derive(Debug, Clone, Deserialize)]
pub struct LlmResponse {
    pub text: String,
    pub expression: String,
}

// ── backend enum ─────────────────────────────────────────

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

// ── system prompt builder ────────────────────────────────

/// Build the full system prompt from the user's persona description and the
/// expression files found on disk.  Users never touch format details.
pub fn build_system_prompt(
    name: &str,
    persona_description: &str,
    expressions_dir: &Path,
) -> String {
    let expressions = scan_expressions(expressions_dir);
    let list = expressions.join(", ");
    format!(
        "\
You are {name}.\n\
{persona_description}\n\
\n\
OUTPUT FORMAT — reply with exactly one JSON object, no other text:\n\
{{\"text\": \"<your reaction, 1-2 sentences>\", \"expression\": \"<one of: {list}>\"}}\n\
Choose the expression that best matches your reaction."
    )
}

/// Scan the expressions directory for available expression names.
fn scan_expressions(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = dir
        .read_dir()
        .ok()
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let p = e.path();
            if p.is_file() {
                let name = p.file_name()?.to_str()?;
                let stem = name.strip_suffix(".osf.bin")?;
                Some(stem.to_string())
            } else {
                None
            }
        })
        .filter(|n| n != "neutral") // neutral is implicit, skip
        .collect();
    names.sort();
    if names.is_empty() {
        names.push("neutral".into());
    }
    names
}

// ── Ollama ───────────────────────────────────────────────

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

// ── OpenAI-compatible ────────────────────────────────────

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
        // Instruct compliant backends to return a JSON object directly.
        // Non-compliant backends ignore this field; parse_response handles
        // the fallback extraction in either case.
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

// ── response parser ──────────────────────────────────────

pub fn parse_response(raw: &str) -> Option<LlmResponse> {
    // jsonrepair handles: markdown fences, unquoted keys, single quotes,
    // trailing commas, truncated JSON, prose-wrapped objects, and more.
    // Falls back to neutral if the output is unrecoverable garbage.
    match jsonrepair_rs::jsonrepair_parse::<LlmResponse>(raw) {
        Ok(r) => Some(r),
        Err(_) => {
            eprintln!("[llm] parse_response: jsonrepair could not recover a valid LlmResponse\n  raw: {raw:?}");
            Some(LlmResponse {
                text: raw.lines().take(2).collect::<Vec<_>>().join(" "),
                expression: "neutral".into(),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::LlmConfig;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

    struct TempExpressionsDir(PathBuf);

    impl TempExpressionsDir {
        fn new() -> Self {
            let id = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "chobits-llm-test-{}-{}",
                std::process::id(),
                id
            ));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).expect("create temp expressions dir");
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }

        fn touch_expression(&self, name: &str) {
            std::fs::write(self.0.join(format!("{name}.osf.bin")), b"x")
                .expect("write expression stub");
        }
    }

    impl Drop for TempExpressionsDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

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

    #[test]
    fn from_config_defaults_to_openai_compatible() {
        let cfg = sample_llm_config("openai");
        match Backend::from_config(&cfg) {
            Backend::OpenAiCompatible { api_key, model, .. } => {
                assert_eq!(model, "test-model");
                assert_eq!(api_key, "secret");
            }
            Backend::Ollama { .. } => panic!("expected openai-compatible backend"),
        }
    }

    #[test]
    fn scan_expressions_excludes_neutral_and_sorts() {
        let dir = TempExpressionsDir::new();
        dir.touch_expression("happy");
        dir.touch_expression("neutral");
        dir.touch_expression("blink");

        assert_eq!(
            scan_expressions(dir.path()),
            vec!["blink".to_string(), "happy".to_string()]
        );
    }

    #[test]
    fn scan_expressions_empty_dir_yields_neutral() {
        let dir = TempExpressionsDir::new();
        assert_eq!(scan_expressions(dir.path()), vec!["neutral".to_string()]);
    }

    #[test]
    fn build_system_prompt_includes_persona_and_expression_list() {
        let dir = TempExpressionsDir::new();
        dir.touch_expression("happy");
        dir.touch_expression("sad");

        let prompt = build_system_prompt("Chi", "Warm and curious.", dir.path());
        assert!(prompt.contains("You are Chi."));
        assert!(prompt.contains("Warm and curious."));
        assert!(prompt.contains("happy, sad"));
        assert!(!prompt.contains("neutral"));
    }

    #[test]
    fn parse_response_accepts_clean_json() {
        let raw = r#"{"text":"nice work!","expression":"happy"}"#;
        let parsed = parse_response(raw).expect("parsed");
        assert_eq!(parsed.text, "nice work!");
        assert_eq!(parsed.expression, "happy");
    }

    #[test]
    fn parse_response_repairs_markdown_fenced_json() {
        let raw = "```json\n{\"text\":\"hey\",\"expression\":\"blink\"}\n```";
        let parsed = parse_response(raw).expect("parsed");
        assert_eq!(parsed.text, "hey");
        assert_eq!(parsed.expression, "blink");
    }

    #[test]
    fn parse_response_falls_back_when_fields_missing() {
        let raw = r#"{"text":"only text"}"#;
        let parsed = parse_response(raw).expect("fallback");
        assert_eq!(parsed.expression, "neutral");
        assert!(parsed.text.contains("only text"));
    }
}
