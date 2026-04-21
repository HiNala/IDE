//! Serializable settings (API keys live in the OS keychain only; see M19/M28).

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Root settings document version for migrations.
pub const SETTINGS_SCHEMA_VERSION: u32 = 1;

pub const KNOWN_PROVIDER_KEYS: &[&str] = &["openai", "anthropic", "gemini", "ollama", "custom"];

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Settings {
    pub version: u32,
    pub ai: AiSettings,
    pub editor: EditorSettings,
    pub terminal: TerminalSettings,
    pub skills: SkillsSettings,
    #[serde(default)]
    pub extra_skill_dirs: Vec<PathBuf>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AiSettings {
    pub active_provider: Option<String>,
    pub active_model: Option<String>,
    pub providers: HashMap<String, ProviderConfig>,
    #[serde(default = "default_true")]
    pub enabled_summarizer: bool,
    #[serde(default = "default_true")]
    pub enabled_vector_index: bool,
    #[serde(default = "default_max_tokens")]
    pub max_tokens_default: u32,
    pub temperature_default: Option<f32>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ProviderConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub default_model: String,
    #[serde(default)]
    pub base_url: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct EditorSettings {
    #[serde(default = "default_editor_font")]
    pub font_size: f32,
    #[serde(default)]
    pub line_ending: LineEndingPreference,
    #[serde(default)]
    pub trim_trailing_whitespace_on_save: bool,
    #[serde(default = "default_true")]
    pub ensure_newline_at_eof: bool,
    #[serde(default)]
    pub word_wrap: bool,
    #[serde(default = "default_undo_coalesce_ms")]
    pub undo_coalesce_ms: u64,
    /// When `true`, file trees include `.ide/` (metadata, tasks). Default: hidden.
    #[serde(default)]
    pub show_ide_internals_in_explorer: bool,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum LineEndingPreference {
    #[default]
    Auto,
    Lf,
    Crlf,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TerminalSettings {
    pub shell_override: Option<PathBuf>,
    #[serde(default)]
    pub shell_args: Vec<String>,
    #[serde(default = "default_term_font")]
    pub font_size: f32,
    #[serde(default = "default_scrollback")]
    pub scrollback_lines: u32,
    #[serde(default = "default_term_height_pct")]
    pub default_height_pct: f32,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct SkillsSettings {
    /// Only disabled skill ids are persisted; enabled is the default.
    #[serde(default)]
    pub disabled: HashSet<String>,
}

fn default_true() -> bool {
    true
}

fn default_max_tokens() -> u32 {
    4096
}

fn default_editor_font() -> f32 {
    14.0
}

fn default_term_font() -> f32 {
    14.0
}

fn default_scrollback() -> u32 {
    10_000
}

fn default_term_height_pct() -> f32 {
    30.0
}

fn default_undo_coalesce_ms() -> u64 {
    editor_core::undo::COALESCE_MS
}

/// JSON export shape (no secrets). Mirrors [`Settings`] for stable import.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SettingsExport {
    pub version: u32,
    pub ai: AiExport,
    pub editor: EditorSettings,
    pub terminal: TerminalSettings,
    pub skills: SkillsSettings,
    pub extra_skill_dirs: Vec<PathBuf>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AiExport {
    pub active_provider: Option<String>,
    pub active_model: Option<String>,
    pub providers: HashMap<String, ProviderConfig>,
    pub enabled_summarizer: bool,
    pub enabled_vector_index: bool,
    pub max_tokens_default: u32,
    pub temperature_default: Option<f32>,
}

impl From<&Settings> for SettingsExport {
    fn from(s: &Settings) -> Self {
        Self {
            version: s.version,
            ai: AiExport {
                active_provider: s.ai.active_provider.clone(),
                active_model: s.ai.active_model.clone(),
                providers: s.ai.providers.clone(),
                enabled_summarizer: s.ai.enabled_summarizer,
                enabled_vector_index: s.ai.enabled_vector_index,
                max_tokens_default: s.ai.max_tokens_default,
                temperature_default: s.ai.temperature_default,
            },
            editor: s.editor.clone(),
            terminal: s.terminal.clone(),
            skills: s.skills.clone(),
            extra_skill_dirs: s.extra_skill_dirs.clone(),
        }
    }
}

impl From<SettingsExport> for Settings {
    fn from(e: SettingsExport) -> Self {
        Self {
            version: e.version,
            ai: AiSettings {
                active_provider: e.ai.active_provider,
                active_model: e.ai.active_model,
                providers: e.ai.providers,
                enabled_summarizer: e.ai.enabled_summarizer,
                enabled_vector_index: e.ai.enabled_vector_index,
                max_tokens_default: e.ai.max_tokens_default,
                temperature_default: e.ai.temperature_default,
            },
            editor: e.editor,
            terminal: e.terminal,
            skills: e.skills,
            extra_skill_dirs: e.extra_skill_dirs,
        }
    }
}

fn default_provider_map() -> HashMap<String, ProviderConfig> {
    let mut m = HashMap::new();
    m.insert(
        "openai".into(),
        ProviderConfig { enabled: true, default_model: "gpt-4o-mini".into(), base_url: None },
    );
    m.insert(
        "anthropic".into(),
        ProviderConfig {
            enabled: true,
            default_model: "claude-sonnet-4-20250514".into(),
            base_url: None,
        },
    );
    m.insert(
        "gemini".into(),
        ProviderConfig { enabled: false, default_model: "gemini-2.0-flash".into(), base_url: None },
    );
    m.insert(
        "ollama".into(),
        ProviderConfig {
            enabled: true,
            default_model: "llama3.2".into(),
            base_url: Some("http://localhost:11434".into()),
        },
    );
    m.insert(
        "custom".into(),
        ProviderConfig {
            enabled: false,
            default_model: "".into(),
            base_url: Some("http://127.0.0.1:11434/v1".into()),
        },
    );
    m
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            version: SETTINGS_SCHEMA_VERSION,
            ai: AiSettings {
                active_provider: None,
                active_model: None,
                providers: default_provider_map(),
                enabled_summarizer: true,
                enabled_vector_index: true,
                max_tokens_default: default_max_tokens(),
                temperature_default: None,
            },
            editor: EditorSettings {
                font_size: default_editor_font(),
                line_ending: LineEndingPreference::Auto,
                trim_trailing_whitespace_on_save: false,
                ensure_newline_at_eof: true,
                word_wrap: false,
                undo_coalesce_ms: default_undo_coalesce_ms(),
                show_ide_internals_in_explorer: false,
            },
            terminal: TerminalSettings {
                shell_override: None,
                shell_args: Vec::new(),
                font_size: default_term_font(),
                scrollback_lines: default_scrollback(),
                default_height_pct: default_term_height_pct(),
            },
            skills: SkillsSettings::default(),
            extra_skill_dirs: Vec::new(),
        }
    }
}
