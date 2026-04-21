//! M21: `read_metadata`, `write_metadata_note`, and `.ide/tasks.md` tools.

use async_trait::async_trait;
use editor_metadata::{
    blank_sidecar,
    store::MetadataStore,
    tasks::{new_task_id, tasks_path, Task, TaskList, TaskStatus},
    workspace_relative, write_to_markdown,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::{Result, ToolError};
use crate::tool::{parse_input, schema_value, Tool, ToolOutput};
use crate::transaction::{PendingChange, WorkspaceTx};

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReadMetadataInput {
    /// Workspace-relative path to a source file (not `.ide/meta/...`).
    pub path: String,
}

#[derive(Debug)]
pub struct ReadMetadataTool;

impl ReadMetadataTool {
    pub const NAME: &'static str = "read_metadata";
}

#[async_trait]
impl Tool for ReadMetadataTool {
    fn name(&self) -> &str {
        Self::NAME
    }

    fn description(&self) -> &str {
        "Load the metadata sidecar for a source file (.ide/meta/…), or a blank skeleton when none exists."
    }

    fn input_schema(&self) -> serde_json::Value {
        schema_value::<ReadMetadataInput>()
    }

    async fn invoke(
        &self,
        input: serde_json::Value,
        tx: &mut WorkspaceTx,
        _dry_run: bool,
    ) -> Result<ToolOutput, ToolError> {
        let p: ReadMetadataInput = parse_input(input)?;
        let path = tx.canonical_path(&p.path)?;
        let store = MetadataStore::new(tx.workspace_root().to_path_buf());
        let rel = workspace_relative(tx.workspace_root(), &path);
        let text = tx.read_path_text(&path).unwrap_or_default();
        let sc = match store.load(&path).map_err(|e| ToolError::msg(e.to_string()))? {
            Some(s) => s,
            None => blank_sidecar(rel.as_path(), &text, "skeleton"),
        };
        let md = write_to_markdown(&sc).map_err(|e| ToolError::msg(e.to_string()))?;
        Ok(ToolOutput::text(md))
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WriteMetadataNoteInput {
    pub path: String,
    pub note: String,
}

#[derive(Debug)]
pub struct WriteMetadataNoteTool;

impl WriteMetadataNoteTool {
    pub const NAME: &'static str = "write_metadata_note";
}

#[async_trait]
impl Tool for WriteMetadataNoteTool {
    fn name(&self) -> &str {
        Self::NAME
    }

    fn description(&self) -> &str {
        "Append to the sidecar Notes section (staged upsert under `.ide/meta/`)."
    }

    fn input_schema(&self) -> serde_json::Value {
        schema_value::<WriteMetadataNoteInput>()
    }

    async fn invoke(
        &self,
        input: serde_json::Value,
        tx: &mut WorkspaceTx,
        dry_run: bool,
    ) -> Result<ToolOutput, ToolError> {
        let p: WriteMetadataNoteInput = parse_input(input)?;
        let path = tx.canonical_path(&p.path)?;
        let store = MetadataStore::new(tx.workspace_root().to_path_buf());
        let rel = workspace_relative(tx.workspace_root(), &path);
        let text = tx.read_path_text(&path).unwrap_or_default();
        let mut sc = match store.load(&path).map_err(|e| ToolError::msg(e.to_string()))? {
            Some(s) => s,
            None => blank_sidecar(rel.as_path(), &text, "manual"),
        };
        if !sc.body.notes.is_empty() {
            sc.body.notes.push_str("\n\n");
        }
        sc.body.notes.push_str(p.note.trim());
        sc.frontmatter.last_updated = chrono::Utc::now();
        let md = write_to_markdown(&sc).map_err(|e| ToolError::msg(e.to_string()))?;
        let sidecar_abs = store.sidecar_path(&path);
        if dry_run {
            return Ok(ToolOutput::text(format!("Would write {}.", sidecar_abs.display())));
        }
        tx.stage_change(PendingChange::UpsertFile { path: sidecar_abs, contents: md });
        Ok(ToolOutput::text("Staged metadata note upsert."))
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, Default)]
pub struct ListTasksInput {}

#[derive(Debug)]
pub struct ListTasksTool;

impl ListTasksTool {
    pub const NAME: &'static str = "list_tasks";
}

#[async_trait]
impl Tool for ListTasksTool {
    fn name(&self) -> &str {
        Self::NAME
    }

    fn description(&self) -> &str {
        "List tasks from `.ide/tasks.md` (JSON array)."
    }

    fn input_schema(&self) -> serde_json::Value {
        schema_value::<ListTasksInput>()
    }

    async fn invoke(
        &self,
        input: serde_json::Value,
        tx: &mut WorkspaceTx,
        _dry_run: bool,
    ) -> Result<ToolOutput, ToolError> {
        let _: ListTasksInput = parse_input(input)?;
        let tp = tasks_path(tx.workspace_root());
        if !tp.is_file() {
            return Ok(ToolOutput::text("[]"));
        }
        let raw = std::fs::read_to_string(&tp).map_err(|e| ToolError::msg(e.to_string()))?;
        let list = TaskList::parse(&raw).map_err(|e| ToolError::msg(e.to_string()))?;
        let j = serde_json::to_string_pretty(&list.entries)
            .map_err(|e| ToolError::msg(e.to_string()))?;
        Ok(ToolOutput::text(j))
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AddTaskInput {
    pub summary: String,
}

#[derive(Debug)]
pub struct AddTaskTool;

impl AddTaskTool {
    pub const NAME: &'static str = "add_task";
}

#[async_trait]
impl Tool for AddTaskTool {
    fn name(&self) -> &str {
        Self::NAME
    }

    fn description(&self) -> &str {
        "Append an open task to `.ide/tasks.md`."
    }

    fn input_schema(&self) -> serde_json::Value {
        schema_value::<AddTaskInput>()
    }

    async fn invoke(
        &self,
        input: serde_json::Value,
        tx: &mut WorkspaceTx,
        dry_run: bool,
    ) -> Result<ToolOutput, ToolError> {
        let p: AddTaskInput = parse_input(input)?;
        let tp = tasks_path(tx.workspace_root());
        let mut list = if tp.is_file() {
            let raw = std::fs::read_to_string(&tp).map_err(|e| ToolError::msg(e.to_string()))?;
            TaskList::parse(&raw).map_err(|e| ToolError::msg(e.to_string()))?
        } else {
            TaskList::empty()
        };
        let id = new_task_id(&p.summary);
        list.entries.push(Task {
            id: id.clone(),
            summary: p.summary,
            status: TaskStatus::Open,
            notes: None,
        });
        let md = list.to_markdown();
        if dry_run {
            return Ok(ToolOutput::text(format!("Would add task `{id}`.")));
        }
        if let Some(parent) = tp.parent() {
            std::fs::create_dir_all(parent).map_err(|e| ToolError::msg(e.to_string()))?;
        }
        tx.stage_change(PendingChange::UpsertFile { path: tp, contents: md });
        Ok(ToolOutput::text("Staged add_task."))
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CompleteTaskInput {
    pub id: String,
}

#[derive(Debug)]
pub struct CompleteTaskTool;

impl CompleteTaskTool {
    pub const NAME: &'static str = "complete_task";
}

#[async_trait]
impl Tool for CompleteTaskTool {
    fn name(&self) -> &str {
        Self::NAME
    }

    fn description(&self) -> &str {
        "Mark a task done by id in `.ide/tasks.md`."
    }

    fn input_schema(&self) -> serde_json::Value {
        schema_value::<CompleteTaskInput>()
    }

    async fn invoke(
        &self,
        input: serde_json::Value,
        tx: &mut WorkspaceTx,
        dry_run: bool,
    ) -> Result<ToolOutput, ToolError> {
        let p: CompleteTaskInput = parse_input(input)?;
        let tp = tasks_path(tx.workspace_root());
        if !tp.is_file() {
            return Err(ToolError::msg("no .ide/tasks.md"));
        }
        let raw = std::fs::read_to_string(&tp).map_err(|e| ToolError::msg(e.to_string()))?;
        let mut list = TaskList::parse(&raw).map_err(|e| ToolError::msg(e.to_string()))?;
        let mut found = false;
        for e in &mut list.entries {
            if e.id == p.id {
                e.status = TaskStatus::Done;
                found = true;
                break;
            }
        }
        if !found {
            return Err(ToolError::msg(format!("unknown task id {}", p.id)));
        }
        let md = list.to_markdown();
        if dry_run {
            return Ok(ToolOutput::text("Would mark done."));
        }
        tx.stage_change(PendingChange::UpsertFile { path: tp, contents: md });
        Ok(ToolOutput::text("Staged complete_task."))
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateTaskInput {
    pub id: String,
    pub status: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug)]
pub struct UpdateTaskTool;

impl UpdateTaskTool {
    pub const NAME: &'static str = "update_task";
}

fn parse_status(s: &str) -> Result<TaskStatus, ToolError> {
    match s {
        "open" => Ok(TaskStatus::Open),
        "in_progress" | "inProgress" => Ok(TaskStatus::InProgress),
        "done" => Ok(TaskStatus::Done),
        "cancelled" | "canceled" => Ok(TaskStatus::Cancelled),
        _ => Err(ToolError::msg("status: open | in_progress | done | cancelled")),
    }
}

#[async_trait]
impl Tool for UpdateTaskTool {
    fn name(&self) -> &str {
        Self::NAME
    }

    fn description(&self) -> &str {
        "Update task status and/or notes by id."
    }

    fn input_schema(&self) -> serde_json::Value {
        schema_value::<UpdateTaskInput>()
    }

    async fn invoke(
        &self,
        input: serde_json::Value,
        tx: &mut WorkspaceTx,
        dry_run: bool,
    ) -> Result<ToolOutput, ToolError> {
        let p: UpdateTaskInput = parse_input(input)?;
        let tp = tasks_path(tx.workspace_root());
        if !tp.is_file() {
            return Err(ToolError::msg("no .ide/tasks.md"));
        }
        let raw = std::fs::read_to_string(&tp).map_err(|e| ToolError::msg(e.to_string()))?;
        let mut list = TaskList::parse(&raw).map_err(|e| ToolError::msg(e.to_string()))?;
        let mut found = false;
        for e in &mut list.entries {
            if e.id == p.id {
                if let Some(ref st) = p.status {
                    e.status = parse_status(st)?;
                }
                if let Some(n) = p.notes {
                    e.notes = if n.is_empty() { None } else { Some(n) };
                }
                found = true;
                break;
            }
        }
        if !found {
            return Err(ToolError::msg(format!("unknown task id {}", p.id)));
        }
        let md = list.to_markdown();
        if dry_run {
            return Ok(ToolOutput::text("Would update."));
        }
        tx.stage_change(PendingChange::UpsertFile { path: tp, contents: md });
        Ok(ToolOutput::text("Staged update_task."))
    }
}
