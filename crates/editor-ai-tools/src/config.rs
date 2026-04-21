//! `.ide/tools.toml` — opt-in shell and prefix whitelist.

use std::fs;
use std::path::Path;

use serde::Deserialize;

use crate::error::{Result, ToolError};

fn default_shell_enabled() -> bool {
    false
}

fn default_allowed_prefixes() -> Vec<String> {
    vec![
        "cargo".into(),
        "npm".into(),
        "pnpm".into(),
        "yarn".into(),
        "python".into(),
        "pytest".into(),
        "make".into(),
        "ls".into(),
        "cat".into(),
        "echo".into(),
    ]
}

fn default_timeout_seconds() -> u32 {
    30
}

/// Shell subsection of [`ToolConfig`].
#[derive(Debug, Clone, Deserialize)]
pub struct ShellSection {
    #[serde(default = "default_shell_enabled")]
    pub enabled: bool,
    #[serde(default = "default_allowed_prefixes")]
    pub allowed_prefixes: Vec<String>,
}

impl Default for ShellSection {
    fn default() -> Self {
        Self { enabled: default_shell_enabled(), allowed_prefixes: default_allowed_prefixes() }
    }
}

/// Top-level config for M20 tools (back-compat: missing file → defaults).
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ToolConfig {
    #[serde(default)]
    pub shell: ShellSection,
}

impl ToolConfig {
    /// Load from `.ide/tools.toml` under the workspace root, or defaults when missing.
    pub fn load_from_workspace_root(workspace_root: &Path) -> Result<Self> {
        let p = workspace_root.join(".ide/tools.toml");
        if !p.is_file() {
            return Ok(Self::default());
        }
        let raw =
            fs::read_to_string(&p).map_err(|e| ToolError::msg(format!("{}: {e}", p.display())))?;
        toml::from_str(&raw).map_err(|e| ToolError::msg(format!("{}: {e}", p.display())))
    }
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RunShellInput {
    pub command: String,
    pub cwd: Option<String>,
    #[serde(default)]
    pub timeout_seconds: Option<u32>,
}

impl RunShellInput {
    #[must_use]
    pub fn timeout_or_default(&self) -> u32 {
        self.timeout_seconds.unwrap_or_else(default_timeout_seconds)
    }
}
