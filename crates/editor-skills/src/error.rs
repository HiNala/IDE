//! Errors from skill parsing and I/O.

use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum SkillParseError {
    #[error("skill markdown must start with YAML frontmatter (---)")]
    NoFrontmatter,
    #[error("missing closing --- after YAML frontmatter")]
    UnclosedFrontmatter,
    #[error("YAML parse: {0}")]
    Yaml(#[from] serde_yaml::Error),
    #[error("skill frontmatter missing required field: {0}")]
    MissingField(&'static str),
}

#[derive(Debug, Error)]
pub enum SkillLoadError {
    #[error("skill not found: {0}")]
    NotFound(String),
    #[error("skill is disabled: {0}")]
    Disabled(String),
    #[error("I/O: {0}")]
    Io(#[from] std::io::Error),
    #[error("parse: {0}")]
    Parse(#[from] SkillParseError),
    #[error("reference path escapes skill directory: {}", .0.display())]
    PathEscape(PathBuf),
}
