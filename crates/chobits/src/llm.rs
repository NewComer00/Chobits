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
    Ollama { url: String, model: String, max_tokens: u32 },
    OpenAiCompatible { url: String, model: String, api_key: String, max_tokens: u32 },
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
            Backend::Ollama { url, model, max_tokens } => query_ollama(url, model, *max_tokens, prompt),
            Backend::OpenAiCompatible { url, model, api_key, max_tokens } => {
                query_openai(url, model, api_key, *max_tokens, prompt)
            }
        }
    }
}

// ── system prompt builder ────────────────────────────────

/// Build the full system prompt from the user's persona description and the
/// expression files found on disk.  Users never touch format details.
pub fn build_system_prompt(name: &str, persona_description: &str, expressions_dir: &Path) -> String {
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

fn query_openai(url: &str, model: &str, api_key: &str, max_tokens: u32, prompt: &str) -> Option<String> {
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
