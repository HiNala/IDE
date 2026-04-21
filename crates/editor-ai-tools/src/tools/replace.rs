//! `replace_in_file` — exact string replace with occurrence control.

use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::{Result, ToolError};
use crate::tool::{parse_input, schema_value, Tool, ToolOutput};
use crate::transaction::{BufferEdit, PendingChange, WorkspaceTx};

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReplaceInFileInput {
    pub path: String,
    pub old_text: String,
    pub new_text: String,
    /// 1 = first match; `0` = replace all occurrences.
    pub occurrence: Option<u32>,
}

#[derive(Debug)]
pub struct ReplaceInFileTool;

impl ReplaceInFileTool {
    pub const NAME: &'static str = "replace_in_file";
}

#[async_trait]
impl Tool for ReplaceInFileTool {
    fn name(&self) -> &str {
        Self::NAME
    }

    fn description(&self) -> &str {
        "Exact string replacement (not regex). Use occurrence=0 for all; default first match only."
    }

    fn input_schema(&self) -> serde_json::Value {
        schema_value::<ReplaceInFileInput>()
    }

    async fn invoke(
        &self,
        input: serde_json::Value,
        tx: &mut WorkspaceTx,
        dry_run: bool,
    ) -> Result<ToolOutput, ToolError> {
        let p: ReplaceInFileInput = parse_input(input)?;
        if p.old_text.is_empty() {
            return Err(ToolError::msg("old_text must not be empty"));
        }
        let path = tx.canonical_path(&p.path)?;
        let text = tx.read_text_staging_base(&path)?;

        let occ = p.occurrence.unwrap_or(1);
        let matches: Vec<usize> = text.match_indices(&p.old_text).map(|(i, _)| i).collect();
        let n = matches.len();
        if n == 0 {
            return Err(ToolError::msg(
                "old_text not found — use grep to locate the exact snippet first.",
            ));
        }

        if occ == 0 {
            if dry_run {
                return Ok(ToolOutput::text(format!(
                    "Will replace all {n} occurrences ({} bytes each).",
                    p.old_text.len()
                )));
            }
            let buf = text.replace(&p.old_text, &p.new_text);
            tx.stage_change(PendingChange::EditBuffer {
                path,
                edit: BufferEdit::FullReplace { new_text: buf },
            });
            return Ok(ToolOutput::text(format!("Staged replace of all {n} occurrences.")));
        }

        if occ == 1 && n > 1 {
            return Err(ToolError::msg(format!(
                "old_text matched {n} times; pass occurrence in 2..={n} to pick one, or 0 for all"
            )));
        }

        let (start, end) = if occ == 1 {
            (matches[0], matches[0].saturating_add(p.old_text.len()))
        } else {
            let idx = (occ as usize).checked_sub(1).ok_or_else(|| {
                ToolError::msg("occurrence must be 0 (all) or a positive match index")
            })?;
            if idx >= n {
                return Err(ToolError::msg(format!("occurrence {occ} out of range ({n} matches)")));
            }
            let s = matches[idx];
            (s, s + p.old_text.len())
        };

        if dry_run {
            return Ok(ToolOutput::text(format!(
                "Will replace bytes {start}-{end} (one occurrence)."
            )));
        }
        tx.stage_change(PendingChange::EditBuffer {
            path,
            edit: BufferEdit::ReplaceRange {
                start_byte: start,
                end_byte: end,
                new_text: p.new_text,
            },
        });
        Ok(ToolOutput::text("Staged replace_in_file."))
    }
}
