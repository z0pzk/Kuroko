use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProviderKind {
    Gemini,
    OpenAiCompatible,
    Ollama,
    Anthropic,
}

impl ProviderKind {
    pub fn label(&self) -> &'static str {
        match self {
            ProviderKind::Gemini => "GEMINI",
            ProviderKind::OpenAiCompatible => "OPENAI / API",
            ProviderKind::Ollama => "OLLAMA (LOCAL)",
            ProviderKind::Anthropic => "CLAUDE",
        }
    }

    pub fn default_model(&self) -> &'static str {
        match self {
            ProviderKind::Gemini => "gemini-2.5-flash",
            ProviderKind::OpenAiCompatible => "gpt-4o-mini",
            ProviderKind::Ollama => "qwen2.5vl:7b",
            ProviderKind::Anthropic => "claude-sonnet-4-20250514",
        }
    }

    pub fn default_base_url(&self) -> &'static str {
        match self {
            ProviderKind::Gemini => "https://generativelanguage.googleapis.com",
            ProviderKind::OpenAiCompatible => "https://api.openai.com/v1",
            ProviderKind::Ollama => "http://localhost:11434",
            ProviderKind::Anthropic => "https://api.anthropic.com",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub provider: ProviderKind,
    pub model: String,
    pub api_key: String,
    pub base_url: String,
    pub target_window_query: String,
    pub max_steps: usize,
    pub step_delay_ms: u64,
    pub humanize_mouse: bool,
    pub always_on_top: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            provider: ProviderKind::Gemini,
            model: ProviderKind::Gemini.default_model().to_string(),
            api_key: String::new(),
            base_url: ProviderKind::Gemini.default_base_url().to_string(),
            target_window_query: "Brave".to_string(),
            max_steps: 15,
            step_delay_ms: 850,
            humanize_mouse: true,
            always_on_top: true,
        }
    }
}

impl AppConfig {
    fn config_path() -> Option<PathBuf> {
        ProjectDirs::from("dev", "kuroko", "kuroko").map(|dirs| dirs.config_dir().join("config.json"))
    }

    pub fn load() -> Self {
        let Some(path) = Self::config_path() else {
            return Self::default();
        };
        match fs::read_to_string(&path) {
            Ok(raw) => serde_json::from_str(&raw).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    pub fn save(&self) {
        let Some(path) = Self::config_path() else {
            return;
        };
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = fs::write(path, json);
        }
    }
}
