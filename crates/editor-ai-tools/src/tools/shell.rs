//! `run_shell` — **non-transactional**, opt-in, prefix allow-list.

use std::time::Duration;

use async_trait::async_trait;
use serde::Serialize;

use crate::config::{RunShellInput, ToolConfig};
use crate::error::{Result, ToolError};
use crate::tool::{parse_input, schema_value, Tool, ToolOutput};
use crate::transaction::WorkspaceTx;

const MAX_CAPTURE: usize = 100 * 1024;

#[derive(Debug)]
pub struct RunShellTool {
    config: ToolConfig,
}

impl RunShellTool {
    pub const NAME: &'static str = "run_shell";

    #[must_use]
    pub fn new(config: ToolConfig) -> Self {
        Self { config }
    }

    fn is_allowed(&self, command: &str) -> bool {
        let cmd = command.trim_start();
        if cmd.is_empty() {
            return false;
        }
        for prefix in &self.config.shell.allowed_prefixes {
            let p = prefix.trim();
            if p.is_empty() {
                continue;
            }
            if cmd == p || cmd.starts_with(&format!("{p} ")) {
                return true;
            }
        }
        false
    }
}

#[derive(Serialize)]
struct RunShellStructured {
    exit_code: Option<i32>,
    stdout: String,
    stderr: String,
}

#[async_trait]
impl Tool for RunShellTool {
    fn name(&self) -> &str {
        Self::NAME
    }

    fn description(&self) -> &str {
        "Run a shell command with a workspace-relative cwd (optional). **Non-transactional**: \
         cannot be rolled back. Disabled unless `[shell] enabled = true` in `.ide/tools.toml` \
         and the command starts with an allowed prefix."
    }

    fn input_schema(&self) -> serde_json::Value {
        schema_value::<RunShellInput>()
    }

    async fn invoke(
        &self,
        input: serde_json::Value,
        tx: &mut WorkspaceTx,
        _dry_run: bool,
    ) -> Result<ToolOutput, ToolError> {
        if !self.config.shell.enabled {
            return Err(ToolError::ShellDenied(
                "shell tool is disabled; set [shell] enabled = true in `.ide/tools.toml`".into(),
            ));
        }
        let p: RunShellInput = parse_input(input)?;
        if !self.is_allowed(&p.command) {
            return Err(ToolError::ShellDenied(format!(
                "command not allowed by prefix whitelist: {:?}",
                self.config.shell.allowed_prefixes
            )));
        }
        let cwd = match &p.cwd {
            None => tx.workspace_root().to_path_buf(),
            Some(s) => tx.canonical_path(s)?,
        };

        let command = p.command.clone();
        let timeout_secs = p.timeout_or_default().max(1);

        let output = tokio::time::timeout(
            Duration::from_secs(timeout_secs as u64),
            tokio::task::spawn_blocking(move || {
                if cfg!(windows) {
                    std::process::Command::new("cmd")
                        .args(["/C", &command])
                        .current_dir(&cwd)
                        .output()
                } else {
                    std::process::Command::new("sh")
                        .arg("-c")
                        .arg(&command)
                        .current_dir(&cwd)
                        .output()
                }
            }),
        )
        .await;

        let output = match output {
            Err(_) => {
                return Err(ToolError::msg(format!("command timed out after {timeout_secs}s")));
            }
            Ok(join) => join
                .map_err(|e| ToolError::msg(format!("shell join: {e}")))?
                .map_err(ToolError::Io)?,
        };

        fn cap_bytes(s: &[u8]) -> String {
            let n = s.len().min(MAX_CAPTURE);
            String::from_utf8_lossy(&s[..n]).into_owned()
        }

        let stdout = cap_bytes(&output.stdout);
        let stderr = cap_bytes(&output.stderr);
        let exit = output.status.code();

        let structured = serde_json::to_value(RunShellStructured {
            exit_code: exit,
            stdout: stdout.clone(),
            stderr: stderr.clone(),
        })
        .unwrap_or_else(|_| serde_json::json!({}));

        let summary = format!("exit={exit:?}\n--- stdout ---\n{stdout}\n--- stderr ---\n{stderr}",);
        Ok(ToolOutput { content: summary, structured: Some(structured), is_error: exit != Some(0) })
    }
}
