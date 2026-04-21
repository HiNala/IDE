//! Read-only tools: `read_file`, `list_directory`, `find_files`, `grep`.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use async_trait::async_trait;
use editor_core::WorkerPool;
use editor_search::{start_project_search, ProjectSearch, SearchEvent};
use editor_workspace::BufferManager;
use editor_workspace::Workspace;
use globset::{GlobBuilder, GlobSetBuilder};
use ignore::WalkBuilder;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::{Result, ToolError};
use crate::tool::{parse_input, schema_value, Tool, ToolOutput};
use crate::transaction::WorkspaceTx;

/// Maximum bytes [`ReadFileTool`] will return (use `grep` for large/binary files).
pub const MAX_READ_FILE_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReadFileInput {
    pub path: String,
    pub start_line: Option<u64>,
    pub end_line: Option<u64>,
}

#[derive(Debug)]
pub struct ReadFileTool;

impl ReadFileTool {
    pub const NAME: &'static str = "read_file";

    fn extract(text: &str, start: Option<u64>, end: Option<u64>) -> Result<(String, usize, usize)> {
        let lines: Vec<&str> = if text.is_empty() { vec![] } else { text.split('\n').collect() };
        let n = lines.len().max(1);
        let s = start.map(|v| v as usize).filter(|&v| v > 0).unwrap_or(1);
        let e = end.map(|v| v as usize).unwrap_or(n).min(n).max(s);
        if s == 0 {
            return Err(ToolError::msg("start_line must be >= 1"));
        }
        if e < s {
            return Err(ToolError::msg("end_line must be >= start_line"));
        }
        let body = lines[s - 1..e].join("\n");
        Ok((body, s, e))
    }
}

#[async_trait]
impl Tool for ReadFileTool {
    fn name(&self) -> &str {
        Self::NAME
    }

    fn description(&self) -> &str {
        "Read a UTF-8 text file under the workspace. Optional 1-based inclusive line range. \
         Rejects files larger than 1 MiB (use grep instead)."
    }

    fn input_schema(&self) -> serde_json::Value {
        schema_value::<ReadFileInput>()
    }

    async fn invoke(
        &self,
        input: serde_json::Value,
        tx: &mut WorkspaceTx,
        _dry_run: bool,
    ) -> Result<ToolOutput, ToolError> {
        let p: ReadFileInput = parse_input(input)?;
        let path = tx.canonical_path(&p.path)?;
        let full = tx.read_path_text(&path)?;
        if full.len() > MAX_READ_FILE_BYTES {
            return Err(ToolError::FileTooLarge { path, max: MAX_READ_FILE_BYTES });
        }
        let (body, a, b) = Self::extract(&full, p.start_line, p.end_line)?;
        let rel = p.path.trim().to_string();
        let header = format!("<file path=\"{}\" lines=\"{a}-{b}\">", rel.replace('\"', "'"));
        Ok(ToolOutput::text(format!("{header}\n{body}\n</file>")))
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ListDirectoryInput {
    pub path: String,
    #[serde(default)]
    pub recursive: bool,
    pub max_depth: Option<u32>,
}

#[derive(Debug)]
pub struct ListDirectoryTool {
    workspace: Arc<Workspace>,
}

impl ListDirectoryTool {
    pub const NAME: &'static str = "list_directory";

    pub fn new(workspace: Arc<Workspace>) -> Self {
        Self { workspace }
    }
}

#[async_trait]
impl Tool for ListDirectoryTool {
    fn name(&self) -> &str {
        Self::NAME
    }

    fn description(&self) -> &str {
        "List files and directories under a workspace-relative path (gitignore-aware)."
    }

    fn input_schema(&self) -> serde_json::Value {
        schema_value::<ListDirectoryInput>()
    }

    async fn invoke(
        &self,
        input: serde_json::Value,
        tx: &mut WorkspaceTx,
        _dry_run: bool,
    ) -> Result<ToolOutput, ToolError> {
        let p: ListDirectoryInput = parse_input(input)?;
        let dir = tx.canonical_path(&p.path)?;
        if !dir.is_dir() {
            return Err(ToolError::msg(format!("not a directory: {}", dir.display())));
        }
        let mut wb = WalkBuilder::new(&dir);
        wb.standard_filters(true);
        if !p.recursive {
            wb.max_depth(Some(1));
        } else if let Some(md) = p.max_depth {
            wb.max_depth(Some(md.saturating_add(1) as usize));
        }
        let mut rows = Vec::new();
        for entry in wb.build() {
            let entry = entry.map_err(|e| ToolError::msg(e.to_string()))?;
            let path = entry.path();
            if path == dir {
                continue;
            }
            if self.workspace.is_ignored(path) {
                continue;
            }
            let meta = entry.metadata().ok();
            let (kind, size) = if path.is_dir() {
                ("directory", 0_u64)
            } else {
                ("file", meta.map(|m| m.len()).unwrap_or(0))
            };
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
            rows.push(serde_json::json!({
                "name": name,
                "path": path.to_string_lossy(),
                "type": kind,
                "size": size,
            }));
        }
        let n = rows.len();
        let v = serde_json::Value::Array(rows);
        Ok(ToolOutput::json(format!("{n} entries"), v))
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FindFilesInput {
    pub pattern: String,
}

#[derive(Debug)]
pub struct FindFilesTool {
    workspace: Arc<Workspace>,
}

impl FindFilesTool {
    pub const NAME: &'static str = "find_files";

    pub fn new(workspace: Arc<Workspace>) -> Self {
        Self { workspace }
    }
}

#[async_trait]
impl Tool for FindFilesTool {
    fn name(&self) -> &str {
        Self::NAME
    }

    fn description(&self) -> &str {
        "Find files matching a glob pattern, respecting gitignore (max 500 results)."
    }

    fn input_schema(&self) -> serde_json::Value {
        schema_value::<FindFilesInput>()
    }

    async fn invoke(
        &self,
        input: serde_json::Value,
        tx: &mut WorkspaceTx,
        _dry_run: bool,
    ) -> Result<ToolOutput, ToolError> {
        let p: FindFilesInput = parse_input(input)?;
        let glob = GlobBuilder::new(&p.pattern)
            .build()
            .map_err(|e| ToolError::msg(format!("invalid glob: {e}")))?;
        let mut gs = GlobSetBuilder::new();
        gs.add(glob);
        let matcher = gs.build().map_err(|e| ToolError::msg(format!("invalid glob set: {e}")))?;

        let root = tx.workspace_root().to_path_buf();
        let mut wb = WalkBuilder::new(&root);
        wb.standard_filters(true);
        let mut out: Vec<String> = Vec::new();

        for entry in wb.build() {
            let entry = entry.map_err(|e| ToolError::msg(e.to_string()))?;
            let path = entry.path();
            if self.workspace.is_ignored(path) {
                continue;
            }
            if !path.is_file() {
                continue;
            }
            let rel = path.strip_prefix(&root).unwrap_or(path);
            if matcher.is_match(rel) {
                out.push(rel.to_string_lossy().into_owned());
                if out.len() >= 500 {
                    break;
                }
            }
        }
        Ok(ToolOutput::json(
            format!("{} matches (cap 500)", out.len()),
            serde_json::Value::Array(out.iter().map(|s| serde_json::json!(s)).collect()),
        ))
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct GrepInput {
    pub query: String,
    #[serde(default)]
    pub is_regex: bool,
    #[serde(default = "default_true")]
    pub case_sensitive: bool,
    pub path: Option<String>,
}

fn default_true() -> bool {
    true
}

#[derive(Debug)]
pub struct GrepTool {
    workspace: Arc<Workspace>,
    pool: Arc<WorkerPool>,
    buffers: Arc<RwLock<BufferManager>>,
}

impl GrepTool {
    pub const NAME: &'static str = "grep";

    pub fn new(
        workspace: Arc<Workspace>,
        pool: Arc<WorkerPool>,
        buffers: Arc<RwLock<BufferManager>>,
    ) -> Self {
        Self { workspace, pool, buffers }
    }
}

#[async_trait]
impl Tool for GrepTool {
    fn name(&self) -> &str {
        Self::NAME
    }

    fn description(&self) -> &str {
        "Search file contents in the workspace (dirty buffers override disk)."
    }

    fn input_schema(&self) -> serde_json::Value {
        schema_value::<GrepInput>()
    }

    async fn invoke(
        &self,
        input: serde_json::Value,
        tx: &mut WorkspaceTx,
        _dry_run: bool,
    ) -> Result<ToolOutput, ToolError> {
        let p: GrepInput = parse_input(input)?;
        let path_prefix = match &p.path {
            None => None,
            Some(s) => Some(tx.canonical_path(s)?),
        };
        let mem: HashMap<PathBuf, String> = {
            let g = self
                .buffers
                .read()
                .map_err(|e| ToolError::msg(format!("buffer lock poisoned: {e}")))?;
            g.dirty_path_contents()
        };
        let workspace = Arc::clone(&self.workspace);
        let pool = Arc::clone(&self.pool);
        let params = ProjectSearch {
            query: p.query,
            is_regex: p.is_regex,
            case_sensitive: p.case_sensitive,
            whole_word: false,
        };
        let out = tokio::task::spawn_blocking(move || {
            let job = start_project_search(params, &workspace, mem, &pool);
            let mut rows = Vec::new();
            while let Ok(ev) = job.rx.recv() {
                match ev {
                    SearchEvent::Match(m) => {
                        if let Some(pref) = &path_prefix {
                            if !m.path.starts_with(pref) {
                                continue;
                            }
                        }
                        rows.push(serde_json::json!({
                            "path": m.path.to_string_lossy(),
                            "line": m.line + 1,
                            "column": m.col_start + 1,
                            "content": m.line_content,
                        }));
                    }
                    SearchEvent::Done { .. } => break,
                    SearchEvent::Error(e) => return Err(ToolError::Search(e)),
                    _ => {}
                }
            }
            Ok::<_, ToolError>(rows)
        })
        .await
        .map_err(|e| ToolError::msg(format!("grep join: {e}")))??;

        let n = out.len();
        Ok(ToolOutput::json(format!("{n} matches"), serde_json::Value::Array(out)))
    }
}
