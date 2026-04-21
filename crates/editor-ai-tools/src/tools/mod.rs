//! Built-in coding-agent tools (M20).

mod edit;
mod explore;
mod file_ops;
mod metadata_tasks;
mod replace;
mod shell;
mod skills;

pub use edit::{AppendToTool, EditLinesTool, InsertAtTool};
pub use explore::{FindFilesTool, GrepTool, ListDirectoryTool, ReadFileTool};
pub use file_ops::{CreateFileTool, DeleteFileTool, MoveFileTool};
pub use metadata_tasks::{
    AddTaskTool, CompleteTaskTool, ListTasksTool, ReadMetadataTool, UpdateTaskTool,
    WriteMetadataNoteTool,
};
pub use replace::ReplaceInFileTool;
pub use shell::RunShellTool;
pub use skills::{ListSkillsTool, LoadSkillReferenceTool, LoadSkillTool};
