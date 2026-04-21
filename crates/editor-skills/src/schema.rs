//! Skill record and YAML frontmatter (Anthropic-compatible `SKILL.md`).

use std::path::PathBuf;

use serde::Deserialize;

/// Where a skill was discovered (precedence: built-in &lt; user global &lt; workspace).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SkillSource {
    Builtin,
    UserGlobal,
    Workspace,
}

/// Parsed `SKILL.md` without filesystem I/O.
#[derive(Debug, Clone)]
pub struct ParsedSkill {
    pub name: String,
    pub description: String,
    pub allowed_tools: Option<Vec<String>>,
    pub disable_model_invocation: bool,
    pub compatibility: Option<String>,
    pub body: String,
}

#[derive(Debug, Clone, Deserialize)]
struct Frontmatter {
    name: String,
    description: String,
    #[serde(default)]
    allowed_tools: Option<Vec<String>>,
    #[serde(default)]
    disable_model_invocation: bool,
    #[serde(default)]
    compatibility: Option<String>,
}

/// One installed skill (metadata + optional static body; dynamic skills omit body).
#[derive(Debug, Clone)]
pub struct Skill {
    pub name: String,
    pub description: String,
    pub source: SkillSource,
    /// Directory containing `SKILL.md` (or empty for synthetic built-ins).
    pub path: PathBuf,
    /// Markdown body without frontmatter; `None` for [`SYSTEM_INFO_SKILL`] (generated on load).
    pub body: Option<String>,
    pub allowed_tools: Option<Vec<String>>,
    pub disable_model_invocation: bool,
    pub compatibility: Option<String>,
}

/// Sentinel name for the dynamic environment skill.
pub const SYSTEM_INFO_SKILL: &str = "system-info";

/// Split `SKILL.md` text into YAML frontmatter and markdown body.
pub fn parse_skill_md(content: &str) -> Result<ParsedSkill, crate::error::SkillParseError> {
    let lines: Vec<&str> = content.lines().collect();
    if lines.first().map(|s| s.trim()) != Some("---") {
        return Err(crate::error::SkillParseError::NoFrontmatter);
    }
    let mut i = 1usize;
    let mut yaml_buf = String::new();
    while i < lines.len() {
        if lines[i].trim() == "---" {
            break;
        }
        yaml_buf.push_str(lines[i]);
        yaml_buf.push('\n');
        i += 1;
    }
    if i >= lines.len() || lines[i].trim() != "---" {
        return Err(crate::error::SkillParseError::UnclosedFrontmatter);
    }
    i += 1;
    let body = if i < lines.len() { lines[i..].join("\n") } else { String::new() };

    let fm: Frontmatter = serde_yaml::from_str(&yaml_buf)?;
    if fm.name.is_empty() {
        return Err(crate::error::SkillParseError::MissingField("name"));
    }
    if fm.description.is_empty() {
        return Err(crate::error::SkillParseError::MissingField("description"));
    }

    Ok(ParsedSkill {
        name: fm.name,
        description: fm.description,
        allowed_tools: fm.allowed_tools,
        disable_model_invocation: fm.disable_model_invocation,
        compatibility: fm.compatibility,
        body,
    })
}
