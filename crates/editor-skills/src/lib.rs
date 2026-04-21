//! Agent skills (`SKILL.md`) parsing and dynamic `system-info` content (M27).

#![forbid(unsafe_code)]

mod builtin;

pub mod error;
pub mod paths;
pub mod registry;
pub mod schema;
pub mod system_info;

pub use error::{SkillLoadError, SkillParseError};
pub use registry::{skill_path_changed, SkillPersistence, SkillRegistry};
pub use schema::{parse_skill_md, ParsedSkill, Skill, SkillSource, SYSTEM_INFO_SKILL};
