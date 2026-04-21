//! [`ToolRegistry`] — name lookup and provider-facing [`editor_ai_provider::ToolDef`] list.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use editor_ai_provider::ToolDef;
use editor_core::WorkerPool;
use editor_skills::SkillRegistry;
use editor_workspace::BufferManager;
use editor_workspace::Workspace;

use crate::config::ToolConfig;
use crate::error::{Result, ToolError};
use crate::tool::{Tool, ToolOutput};
use crate::tools::*;
use crate::transaction::WorkspaceTx;

/// Default M20+M21 tool set (18 tools without skills). `run_shell` obeys `config`.
#[allow(missing_debug_implementations)]
pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
}

impl ToolRegistry {
    /// Registers all built-in tools.
    #[must_use]
    pub fn new_default(
        workspace: &Arc<Workspace>,
        buffers: &Arc<RwLock<BufferManager>>,
        config: &ToolConfig,
        skills: Option<Arc<RwLock<SkillRegistry>>>,
    ) -> Self {
        let pool = Arc::new(WorkerPool::new(Some(2)));
        let mut tools: HashMap<String, Arc<dyn Tool>> = HashMap::new();

        macro_rules! ins {
            ($tool:expr) => {
                let t: Arc<dyn Tool> = Arc::new($tool);
                tools.insert(t.name().to_string(), t);
            };
        }

        ins!(ReadFileTool);
        ins!(ListDirectoryTool::new(Arc::clone(workspace)));
        ins!(FindFilesTool::new(Arc::clone(workspace)));
        ins!(GrepTool::new(Arc::clone(workspace), Arc::clone(&pool), Arc::clone(buffers),));
        ins!(EditLinesTool);
        ins!(InsertAtTool);
        ins!(AppendToTool);
        ins!(ReplaceInFileTool);
        ins!(CreateFileTool);
        ins!(DeleteFileTool);
        ins!(MoveFileTool);
        ins!(ReadMetadataTool);
        ins!(WriteMetadataNoteTool);
        ins!(ListTasksTool);
        ins!(AddTaskTool);
        ins!(CompleteTaskTool);
        ins!(UpdateTaskTool);
        ins!(RunShellTool::new(config.clone()));

        if let Some(sr) = skills {
            ins!(LoadSkillTool::new(Arc::clone(&sr)));
            ins!(ListSkillsTool::new(Arc::clone(&sr)));
            ins!(LoadSkillReferenceTool::new(sr));
        }

        Self { tools }
    }

    /// Schemas and descriptions for [`editor_ai_provider::ChatRequest::tools`].
    #[must_use]
    pub fn as_defs(&self) -> Vec<ToolDef> {
        let mut names: Vec<_> = self.tools.keys().cloned().collect();
        names.sort();
        names
            .into_iter()
            .filter_map(|n| self.tools.get(&n))
            .map(|t| ToolDef {
                name: t.name().to_string(),
                description: t.description().to_string(),
                input_schema: t.input_schema(),
            })
            .collect()
    }

    /// Execute a tool by wire name (provider tool call).
    pub async fn invoke(
        &self,
        name: &str,
        input: serde_json::Value,
        tx: &mut WorkspaceTx,
        dry_run: bool,
    ) -> Result<ToolOutput, ToolError> {
        let t =
            self.tools.get(name).ok_or_else(|| ToolError::msg(format!("unknown tool: {name}")))?;
        t.invoke(input, tx, dry_run).await
    }

    #[must_use]
    pub fn tool_names(&self) -> Vec<String> {
        let mut n: Vec<_> = self.tools.keys().cloned().collect();
        n.sort();
        n
    }
}
