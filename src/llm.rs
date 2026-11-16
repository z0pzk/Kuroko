use crate::config::{AppConfig, ProviderKind};
use anyhow::{anyhow, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum AgentAction {
    Click {
        x: f32,
        y: f32,
        #[serde(default)]
        double: bool,
        #[serde(default)]
        reason: String,
    },
    Type {
        text: String,
        #[serde(default)]
        press_enter: bool,
        #[serde(default)]
        reason: String,
    },
    Key {
        key: String,
        #[serde(default)]
        reason: String,
    },
    Scroll {
        delta: i32,
        #[serde(default)]
        reason: String,
    },
    Wait {
        ms: u64,
        #[serde(default)]
        reason: String,
    },
    Done {
        summary: String,
    },
}

const SYSTEM_PROMPT: &str = r#"You are KUROKO, an autonomous visual browser operator.
You see a screenshot of a Brave browser window. Coordinates are normalized to a 0..1000 grid where (0,0) is top-left and (1000,1000) is bottom-right of the browser window.

Return ONLY a single valid JSON object matching one of these schemas:
1. {"action":"click","x":500,"y":300,"double":false,"reason":"click search bar"}
2. {"action":"type","text":"hello world","press_enter":true,"reason":"type query"}
3. {"action":"key","key":"enter","reason":"submit"}  (keys: enter, tab, escape, backspace, space, up, down, left, right, pagedown, pageup)
4. {"action":"scroll","delta":-5,"reason":"scroll down"} (negative = down, positive = up)
5. {"action":"wait","ms":1000,"reason":"wait for page load"}
6. {"action":"done","summary":"task completed"}

Rules:
- Always click an input field before typing into it unless it is already visibly focused.
- Never output markdown fences or extra commentary—JSON only."#;

pub async fn decide_next_action(
    client: &Client,
    cfg: &AppConfig,
    goal: &str,
    history: &[String],
    screenshot_b64: &str,
) -> Result<AgentAction> {
    let history_text = if history.is_empty() {
        "None yet.".to_string()
    } else {
        history.join("\n")
    };

    let user_prompt = format!(
        "USER GOAL: {goal}\n\nPREVIOUS STEPS:\n{history_text}\n\nInspect the current Brave screenshot and return the next single JSON action."
    );

    let raw_text = match cfg.provider {
        ProviderKind::Gemini => call_gemini(client, cfg, &user_prompt, screenshot_b64).await?,
        ProviderKind::OpenAiCompatible => {
            call_openai_compat(client, cfg, &user_prompt, screenshot_b64).await?
        }
        ProviderKind::Ollama => call_ollama(client, cfg, &user_prompt, screenshot_b64).await?,
        ProviderKind::Anthropic => call_anthropic(client, cfg, &user_prompt, screenshot_b64).await?,
    };

    parse_action_json(&raw_text)
}

async fn call_gemini(
    client: &Client,
    cfg: &AppConfig,
    user_prompt: &str,
    screenshot_b64: &str,
) -> Result<String> {
    if cfg.api_key.trim().is_empty() {
        return Err(anyhow!("Gemini API key is empty"));
    }
    let base = cfg.base_url.trim_end_matches('/');
    let url = format!(
        "{}/v1beta/models/{}:generateContent?key={}",
        base,
        cfg.model.trim(),
        cfg.api_key.trim()
    );

    let body = json!({
        "systemInstruction": {
            "parts": [{ "text": SYSTEM_PROMPT }]
        },
        "contents": [{
            "role": "user",
            "parts": [
                { "inlineData": { "mimeType": "image/jpeg", "data": screenshot_b64 } },
                { "text": user_prompt }
            ]
        }],
        "generationConfig": {
            "temperature": 0.1,
            "responseMimeType": "application/json"
        }
    });

    let resp = client.post(url).json(&body).send().await?;
    let status = resp.status();
    let val: Value = resp.json().await?;
    if !status.is_success() {
        return Err(anyhow!("Gemini API error ({status}): {val}"));
    }

    val["candidates"][0]["content"]["parts"][0]["text"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow!("Unexpected Gemini response format: {val}"))
}

async fn call_openai_compat(
    client: &Client,
    cfg: &AppConfig,
    user_prompt: &str,
    screenshot_b64: &str,
) -> Result<String> {
    let base = cfg.base_url.trim_end_matches('/');
    let url = format!("{base}/chat/completions");

    let body = json!({
        "model": cfg.model.trim(),
        "temperature": 0.1,
        "messages": [
            { "role": "system", "content": SYSTEM_PROMPT },
            {
                "role": "user",
                "content": [
                    { "type": "text", "text": user_prompt },
                    { "type": "image_url", "image_url": { "url": format!("data:image/jpeg;base64,{screenshot_b64}") } }
                ]
            }
        ]
    });

    let mut req = client.post(url).json(&body);
    if !cfg.api_key.trim().is_empty() {
        req = req.bearer_auth(cfg.api_key.trim());
    }

    let resp = req.send().await?;
    let status = resp.status();
    let val: Value = resp.json().await?;
    if !status.is_success() {
        return Err(anyhow!("OpenAI-compat API error ({status}): {val}"));
    }

    val["choices"][0]["message"]["content"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow!("Unexpected OpenAI response format: {val}"))
}

async fn call_ollama(
    client: &Client,
    cfg: &AppConfig,
    user_prompt: &str,
    screenshot_b64: &str,
) -> Result<String> {
    let base = cfg.base_url.trim_end_matches('/');
    let url = format!("{base}/api/chat");

    let body = json!({
        "model": cfg.model.trim(),
        "stream": false,
        "format": "json",
        "messages": [
            { "role": "system", "content": SYSTEM_PROMPT },
            {
                "role": "user",
                "content": user_prompt,
                "images": [screenshot_b64]
            }
        ]
    });

    let resp = client.post(url).json(&body).send().await?;
    let status = resp.status();
    let val: Value = resp.json().await?;
    if !status.is_success() {
        return Err(anyhow!("Ollama API error ({status}): {val}"));
    }

    val["message"]["content"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow!("Unexpected Ollama response format: {val}"))
}

async fn call_anthropic(
    client: &Client,
    cfg: &AppConfig,
    user_prompt: &str,
    screenshot_b64: &str,
) -> Result<String> {
    if cfg.api_key.trim().is_empty() {
        return Err(anyhow!("Anthropic API key is empty"));
    }
    let base = cfg.base_url.trim_end_matches('/');
    let url = format!("{base}/v1/messages");

    let body = json!({
        "model": cfg.model.trim(),
        "max_tokens": 512,
        "system": SYSTEM_PROMPT,
        "messages": [{
            "role": "user",
            "content": [
                {
                    "type": "image",
                    "source": {
                        "type": "base64",
                        "media_type": "image/jpeg",
                        "data": screenshot_b64
                    }
                },
                { "type": "text", "text": user_prompt }
            ]
        }]
    });

    let resp = client
        .post(url)
        .header("x-api-key", cfg.api_key.trim())
        .header("anthropic-version", "2023-06-01")
        .json(&body)
        .send()
        .await?;
    let status = resp.status();
    let val: Value = resp.json().await?;
    if !status.is_success() {
        return Err(anyhow!("Anthropic API error ({status}): {val}"));
    }

    val["content"][0]["text"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow!("Unexpected Anthropic response format: {val}"))
}

fn parse_action_json(raw: &str) -> Result<AgentAction> {
    let trimmed = raw.trim();
    if let Ok(act) = serde_json::from_str::<AgentAction>(trimmed) {
        return Ok(act);
    }

    // Fallback: extract first JSON object if model wrapped it in markdown fences
    if let (Some(start), Some(end)) = (trimmed.find('{'), trimmed.rfind('}')) {
        let slice = &trimmed[start..=end];
        if let Ok(act) = serde_json::from_str::<AgentAction>(slice) {
            return Ok(act);
        }
    }

    Err(anyhow!("Model returned invalid action JSON: {trimmed}"))
}
