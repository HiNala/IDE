//! `load_skill`, `list_skills`, `load_skill_reference` (M27).

use std::sync::{Arc, RwLock};

use async_trait::async_trait;
use schemars::JsonSchema;
use serde::Deserialize;

use editor_skills::SkillRegistry;

use crate::error::{Result, ToolError};
use crate::tool::{parse_input, schema_value, Tool, ToolOutput};
use crate::transaction::WorkspaceTx;

#[derive(Debug, Deserialize, JsonSchema)]
struct LoadSkillInput {
    /// Skill id (e.g. `using-terminal`, `system-info`).
    name: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct LoadSkillReferenceInput {
    skill_name: String,
    /// File relative to the skill directory (e.g. `examples.md`).
    file: String,
}

/// Returns the markdown body for a skill (progressive disclosure).
#[derive(Debug)]
pub struct LoadSkillTool {
    registry: Arc<RwLock<SkillRegistry>>,
}

impl LoadSkillTool {
    pub const NAME: &'static str = "load_skill";

    #[must_use]
    pub fn new(registry: Arc<RwLock<SkillRegistry>>) -> Self {
        Self { registry }
    }
}

#[async_trait]
impl Tool for LoadSkillTool {
    fn name(&self) -> &str {
        Self::NAME
    }

    fn description(&self) -> &str {
        "Load the full markdown instructions for a named skill. Call when a task matches a \
         skill description from the system prompt catalog."
    }

    fn input_schema(&self) -> serde_json::Value {
        schema_value::<LoadSkillInput>()
    }

    async fn invoke(
        &self,
        input: serde_json::Value,
        _tx: &mut WorkspaceTx,
        _dry_run: bool,
    ) -> Result<ToolOutput, ToolError> {
        let p: LoadSkillInput = parse_input(input)?;
        let reg = self.registry.read().map_err(|e: std::sync::PoisonError<_>| {
            ToolError::msg(format!("skills registry lock poisoned: {e}"))
        })?;
        match reg.load_skill_body(&p.name) {
            Ok(body) => Ok(ToolOutput::text(body)),
            Err(e) => Ok(ToolOutput::error(e.to_string())),
        }
    }
}

#[derive(Debug)]
pub struct ListSkillsTool {
    registry: Arc<RwLock<SkillRegistry>>,
}

impl ListSkillsTool {
    pub const NAME: &'static str = "list_skills";

    #[must_use]
    pub fn new(registry: Arc<RwLock<SkillRegistry>>) -> Self {
        Self { registry }
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
struct EmptyObject {}

#[async_trait]
impl Tool for ListSkillsTool {
    fn name(&self) -> &str {
        Self::NAME
    }

    fn description(&self) -> &str {
        "Returns the current `<available_skills>` catalog (same shape as the system prompt)."
    }

    fn input_schema(&self) -> serde_json::Value {
        schema_value::<EmptyObject>()
    }

    async fn invoke(
        &self,
        _input: serde_json::Value,
        _tx: &mut WorkspaceTx,
        _dry_run: bool,
    ) -> Result<ToolOutput, ToolError> {
        let reg = self.registry.read().map_err(|e: std::sync::PoisonError<_>| {
            ToolError::msg(format!("skills registry lock poisoned: {e}"))
        })?;
        Ok(ToolOutput::text(reg.summary_for_system_prompt()))
    }
}

#[derive(Debug)]
pub struct LoadSkillReferenceTool {
    registry: Arc<RwLock<SkillRegistry>>,
}

impl LoadSkillReferenceTool {
    pub const NAME: &'static str = "load_skill_reference";

    #[must_use]
    pub fn new(registry: Arc<RwLock<SkillRegistry>>) -> Self {
        Self { registry }
    }
}

#[async_trait]
impl Tool for LoadSkillReferenceTool {
    fn name(&self) -> &str {
        Self::NAME
    }

    fn description(&self) -> &str {
        "Load an auxiliary file from a skill directory (path-safe; must stay under the skill folder)."
    }

    fn input_schema(&self) -> serde_json::Value {
        schema_value::<LoadSkillReferenceInput>()
    }

    async fn invoke(
        &self,
        input: serde_json::Value,
        _tx: &mut WorkspaceTx,
        _dry_run: bool,
    ) -> Result<ToolOutput, ToolError> {
        let p: LoadSkillReferenceInput = parse_input(input)?;
        let reg = self.registry.read().map_err(|e: std::sync::PoisonError<_>| {
            ToolError::msg(format!("skills registry lock poisoned: {e}"))
        })?;
        match reg.load_skill_reference(&p.skill_name, &p.file) {
            Ok(body) => Ok(ToolOutput::text(body)),
            Err(e) => Ok(ToolOutput::error(e.to_string())),
        }
    }
}
