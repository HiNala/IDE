//! `create_file`, `delete_file`, `move_file`.

use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::{Result, ToolError};
use crate::tool::{parse_input, schema_value, Tool, ToolOutput};
use crate::transaction::{PendingChange, WorkspaceTx};

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateFileInput {
    pub path: String,
    pub content: String,
}

#[derive(Debug)]
pub struct CreateFileTool;

impl CreateFileTool {
    pub const NAME: &'static str = "create_file";
}

#[async_trait]
impl Tool for CreateFileTool {
    fn name(&self) -> &str {
        Self::NAME
    }

    fn description(&self) -> &str {
        "Create a new file under the workspace. Fails if the path already exists."
    }

    fn input_schema(&self) -> serde_json::Value {
        schema_value::<CreateFileInput>()
    }

    async fn invoke(
        &self,
        input: serde_json::Value,
        tx: &mut WorkspaceTx,
        dry_run: bool,
    ) -> Result<ToolOutput, ToolError> {
        let p: CreateFileInput = parse_input(input)?;
        let path = tx.canonical_path(&p.path)?;
        if path.exists() {
            return Err(ToolError::msg(format!("{} already exists", path.display())));
        }
        if dry_run {
            return Ok(ToolOutput::text(format!(
                "Will create {} ({} bytes).",
                path.display(),
                p.content.len()
            )));
        }
        tx.stage_change(PendingChange::WriteNewFile { path, contents: p.content });
        Ok(ToolOutput::text("Staged create_file."))
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeleteFileInput {
    pub path: String,
}

#[derive(Debug)]
pub struct DeleteFileTool;

impl DeleteFileTool {
    pub const NAME: &'static str = "delete_file";
}

#[async_trait]
impl Tool for DeleteFileTool {
    fn name(&self) -> &str {
        Self::NAME
    }

    fn description(&self) -> &str {
        "Delete a file under the workspace (staged until commit)."
    }

    fn input_schema(&self) -> serde_json::Value {
        schema_value::<DeleteFileInput>()
    }

    async fn invoke(
        &self,
        input: serde_json::Value,
        tx: &mut WorkspaceTx,
        dry_run: bool,
    ) -> Result<ToolOutput, ToolError> {
        let p: DeleteFileInput = parse_input(input)?;
        let path = tx.canonical_path(&p.path)?;
        if !path.exists() {
            return Err(ToolError::msg(format!("{} does not exist", path.display())));
        }
        let prior = tx.read_path_text(&path).ok();
        if dry_run {
            return Ok(ToolOutput::text(format!(
                "Will delete {} ({} bytes).",
                path.display(),
                prior.as_ref().map(|s| s.len()).unwrap_or(0)
            )));
        }
        tx.stage_change(PendingChange::DeleteFile { path, prior_contents: prior });
        Ok(ToolOutput::text("Staged delete_file."))
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct MoveFileInput {
    pub from: String,
    pub to: String,
}

#[derive(Debug)]
pub struct MoveFileTool;

impl MoveFileTool {
    pub const NAME: &'static str = "move_file";
}

#[async_trait]
impl Tool for MoveFileTool {
    fn name(&self) -> &str {
        Self::NAME
    }

    fn description(&self) -> &str {
        "Rename or move a file within the workspace."
    }

    fn input_schema(&self) -> serde_json::Value {
        schema_value::<MoveFileInput>()
    }

    async fn invoke(
        &self,
        input: serde_json::Value,
        tx: &mut WorkspaceTx,
        dry_run: bool,
    ) -> Result<ToolOutput, ToolError> {
        let p: MoveFileInput = parse_input(input)?;
        let from = tx.canonical_path(&p.from)?;
        let to = tx.canonical_path(&p.to)?;
        if !from.exists() {
            return Err(ToolError::msg(format!("{} does not exist", from.display())));
        }
        if to.exists() {
            return Err(ToolError::msg(format!("{} already exists", to.display())));
        }
        if dry_run {
            return Ok(ToolOutput::text(format!(
                "Will move {} -> {}.",
                from.display(),
                to.display()
            )));
        }
        tx.stage_change(PendingChange::MoveFile { from, to });
        Ok(ToolOutput::text("Staged move_file."))
    }
}
