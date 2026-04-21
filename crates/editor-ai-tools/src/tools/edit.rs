//! Precision edits: `edit_lines`, `insert_at`, `append_to`.

use async_trait::async_trait;
use editor_core::TextBuffer;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::{Result, ToolError};
use crate::tool::{parse_input, schema_value, Tool, ToolOutput};
use crate::transaction::{BufferEdit, PendingChange, WorkspaceTx};

fn line_replacement_span(
    buf: &TextBuffer,
    start_line: u64,
    end_line: u64,
) -> Result<(usize, usize)> {
    if start_line == 0 || end_line < start_line {
        return Err(ToolError::msg("invalid line range (use 1-based inclusive lines)"));
    }
    let s0 = (start_line - 1) as usize;
    let next_line = end_line as usize;
    let start_byte = buf.line_to_byte(s0).map_err(ToolError::Core)?;
    let total = buf.len_lines();
    let end_byte = if next_line >= total {
        buf.len_bytes()
    } else {
        buf.line_to_byte(next_line).map_err(ToolError::Core)?
    };
    Ok((start_byte, end_byte))
}

fn insert_byte_before_line(buf: &TextBuffer, line: u64) -> Result<usize> {
    if line == 0 {
        return Err(ToolError::msg("line must be >= 1"));
    }
    let idx = (line - 1) as usize;
    buf.line_to_byte(idx).map_err(ToolError::Core)
}

fn preview_range_msg(old_len: usize, new_len: usize, start_line: u64, end_line: u64) -> String {
    let n_lines = end_line.saturating_sub(start_line).saturating_add(1);
    format!(
        "Will replace lines {start_line}-{end_line} ({n_lines} lines, {old_len} bytes) \
         with new content ({new_len} bytes)."
    )
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct EditLinesInput {
    pub path: String,
    pub start_line: u64,
    pub end_line: u64,
    pub new_content: String,
}

#[derive(Debug)]
pub struct EditLinesTool;

impl EditLinesTool {
    pub const NAME: &'static str = "edit_lines";
}

#[async_trait]
impl Tool for EditLinesTool {
    fn name(&self) -> &str {
        Self::NAME
    }

    fn description(&self) -> &str {
        "Replace a 1-based inclusive range of lines with new UTF-8 content (staged until commit)."
    }

    fn input_schema(&self) -> serde_json::Value {
        schema_value::<EditLinesInput>()
    }

    async fn invoke(
        &self,
        input: serde_json::Value,
        tx: &mut WorkspaceTx,
        dry_run: bool,
    ) -> Result<ToolOutput, ToolError> {
        let p: EditLinesInput = parse_input(input)?;
        let path = tx.canonical_path(&p.path)?;
        let text = tx.read_text_staging_base(&path)?;
        let buf = TextBuffer::from_str(&text);
        let (start_b, end_b) = line_replacement_span(&buf, p.start_line, p.end_line)?;
        let removed_len = end_b.saturating_sub(start_b);
        let new_len = p.new_content.len();
        if dry_run {
            return Ok(ToolOutput::text(preview_range_msg(
                removed_len,
                new_len,
                p.start_line,
                p.end_line,
            )));
        }
        tx.stage_change(PendingChange::EditBuffer {
            path,
            edit: BufferEdit::ReplaceRange {
                start_byte: start_b,
                end_byte: end_b,
                new_text: p.new_content,
            },
        });
        Ok(ToolOutput::text(format!("Staged replace for lines {}-{}.", p.start_line, p.end_line)))
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct InsertAtInput {
    pub path: String,
    pub line: u64,
    pub content: String,
}

#[derive(Debug)]
pub struct InsertAtTool;

impl InsertAtTool {
    pub const NAME: &'static str = "insert_at";
}

#[async_trait]
impl Tool for InsertAtTool {
    fn name(&self) -> &str {
        Self::NAME
    }

    fn description(&self) -> &str {
        "Insert text before the given 1-based line number (`line=1` inserts at file start)."
    }

    fn input_schema(&self) -> serde_json::Value {
        schema_value::<InsertAtInput>()
    }

    async fn invoke(
        &self,
        input: serde_json::Value,
        tx: &mut WorkspaceTx,
        dry_run: bool,
    ) -> Result<ToolOutput, ToolError> {
        let p: InsertAtInput = parse_input(input)?;
        let path = tx.canonical_path(&p.path)?;
        let text = tx.read_text_staging_base(&path)?;
        let buf = TextBuffer::from_str(&text);
        let off = insert_byte_before_line(&buf, p.line)?;
        if dry_run {
            return Ok(ToolOutput::text(format!(
                "Will insert {} bytes before line {}.",
                p.content.len(),
                p.line
            )));
        }
        tx.stage_change(PendingChange::EditBuffer {
            path,
            edit: BufferEdit::InsertAt { byte_offset: off, text: p.content },
        });
        Ok(ToolOutput::text("Staged insert_at."))
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AppendToInput {
    pub path: String,
    pub content: String,
}

#[derive(Debug)]
pub struct AppendToTool;

impl AppendToTool {
    pub const NAME: &'static str = "append_to";
}

#[async_trait]
impl Tool for AppendToTool {
    fn name(&self) -> &str {
        Self::NAME
    }

    fn description(&self) -> &str {
        "Append text at end of file. Inserts a leading newline when the file has content but no trailing newline."
    }

    fn input_schema(&self) -> serde_json::Value {
        schema_value::<AppendToInput>()
    }

    async fn invoke(
        &self,
        input: serde_json::Value,
        tx: &mut WorkspaceTx,
        dry_run: bool,
    ) -> Result<ToolOutput, ToolError> {
        let p: AppendToInput = parse_input(input)?;
        let path = tx.canonical_path(&p.path)?;
        let text = tx.read_text_staging_base(&path)?;
        let mut insert = p.content;
        if !text.is_empty() && !text.ends_with('\n') {
            insert.insert(0, '\n');
        }
        let off = text.len();
        if dry_run {
            return Ok(ToolOutput::text(format!(
                "Will append {} bytes at EOF (offset {off}).",
                insert.len()
            )));
        }
        tx.stage_change(PendingChange::EditBuffer {
            path,
            edit: BufferEdit::InsertAt { byte_offset: off, text: insert },
        });
        Ok(ToolOutput::text("Staged append_to."))
    }
}
