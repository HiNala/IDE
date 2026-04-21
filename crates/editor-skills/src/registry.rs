//! SkillRegistry — discovery and system prompt (M27).

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use tracing::{debug, warn};

use crate::builtin::BUILTIN_SKILL_TEXTS;
use crate::error::SkillLoadError;
use crate::paths::user_global_skills_dir;
use crate::schema::{parse_skill_md, Skill, SkillSource, SYSTEM_INFO_SKILL};
use crate::system_info::{generate_system_info_body, invalidate_system_info_cache};

#[derive(Debug, Clone, Default)]
pub struct SkillPersistence {
    pub disabled: HashSet<String>,
    pub extra_dirs: Vec<PathBuf>,
}

#[derive(Debug)]
pub struct SkillRegistry {
    skills: HashMap<String, Skill>,
    disabled: HashSet<String>,
    workspace_root: Option<PathBuf>,
    extra_dirs: Vec<PathBuf>,
}

impl SkillRegistry {
    #[must_use]
    pub fn load(workspace_root: Option<&Path>, persistence: &SkillPersistence) -> Self {
        let mut reg = Self {
            skills: HashMap::new(),
            disabled: persistence.disabled.clone(),
            workspace_root: workspace_root.map(Path::to_path_buf),
            extra_dirs: persistence.extra_dirs.clone(),
        };
        reg.discover_builtin();
        reg.discover_user_global();
        for d in &persistence.extra_dirs.clone() {
            reg.load_dir(d, SkillSource::UserGlobal);
        }
        if let Some(root) = workspace_root {
            reg.load_dir(&root.join(".ide").join("skills"), SkillSource::Workspace);
        }
        reg
    }

    fn discover_builtin(&mut self) {
        for (name, text) in BUILTIN_SKILL_TEXTS {
            match parse_skill_md(text) {
                Ok(p) => self.insert_parsed(p, SkillSource::Builtin, PathBuf::new()),
                Err(e) => warn!(skill = name, error = %e, "builtin skill parse failed"),
            }
        }
        self.skills.insert(
            SYSTEM_INFO_SKILL.to_string(),
            Skill {
                name: SYSTEM_INFO_SKILL.to_string(),
                description: "Current OS, shell, language toolchains, and project metadata. Use whenever any task depends on the environment — paths, compilers, package managers, or git. Always load at the start of environment-dependent work.".into(),
                source: SkillSource::Builtin,
                path: PathBuf::new(),
                body: None,
                allowed_tools: None,
                disable_model_invocation: false,
                compatibility: None,
            },
        );
    }

    fn discover_user_global(&mut self) {
        let Some(dir) = user_global_skills_dir() else {
            return;
        };
        self.load_dir(&dir, SkillSource::UserGlobal);
    }

    fn load_dir(&mut self, base: &Path, source: SkillSource) {
        let Ok(rd) = fs::read_dir(base) else {
            return;
        };
        for ent in rd.flatten() {
            let p = ent.path();
            if !p.is_dir() {
                continue;
            }
            let md = p.join("SKILL.md");
            if !md.is_file() {
                continue;
            }
            match fs::read_to_string(&md) {
                Ok(text) => match parse_skill_md(&text) {
                    Ok(parsed) => self.insert_parsed(parsed, source, p.clone()),
                    Err(e) => warn!(?md, error = %e, "skill parse failed"),
                },
                Err(e) => warn!(?md, error = %e, "skill read failed"),
            }
        }
    }

    fn insert_parsed(&mut self, p: crate::schema::ParsedSkill, source: SkillSource, dir: PathBuf) {
        let skill = Skill {
            name: p.name.clone(),
            description: p.description,
            source,
            path: dir,
            body: Some(p.body),
            allowed_tools: p.allowed_tools,
            disable_model_invocation: p.disable_model_invocation,
            compatibility: p.compatibility,
        };
        debug!(name = %skill.name, ?source, "skill registered");
        self.skills.insert(skill.name.clone(), skill);
    }

    pub fn reload(&mut self, workspace_root: Option<&Path>, persistence: &SkillPersistence) {
        invalidate_system_info_cache();
        self.skills.clear();
        self.disabled = persistence.disabled.clone();
        self.workspace_root = workspace_root.map(Path::to_path_buf);
        self.extra_dirs = persistence.extra_dirs.clone();
        self.discover_builtin();
        self.discover_user_global();
        for d in &self.extra_dirs.clone() {
            self.load_dir(d, SkillSource::UserGlobal);
        }
        if let Some(root) = workspace_root {
            self.load_dir(&root.join(".ide").join("skills"), SkillSource::Workspace);
        }
    }

    #[must_use]
    pub fn list(&self) -> Vec<&Skill> {
        let mut v: Vec<_> = self.skills.values().collect();
        v.sort_by(|a, b| a.name.cmp(&b.name));
        v
    }

    #[must_use]
    pub fn get(&self, name: &str) -> Option<&Skill> {
        self.skills.get(name)
    }

    pub fn enabled_skills(&self) -> impl Iterator<Item = &Skill> {
        self.skills.values().filter(move |s| self.is_enabled(&s.name))
    }

    #[must_use]
    pub fn is_enabled(&self, name: &str) -> bool {
        !self.disabled.contains(name)
    }

    pub fn set_enabled(&mut self, name: &str, enabled: bool) {
        if enabled {
            self.disabled.remove(name);
        } else {
            self.disabled.insert(name.to_string());
        }
    }

    #[must_use]
    pub fn summary_for_system_prompt(&self) -> String {
        let mut out = String::from("<available_skills>\n");
        for s in self.enabled_skills() {
            let esc = xml_escape(&s.description);
            out.push_str(&format!("  <skill name=\"{}\" description=\"{}\" />\n", s.name, esc));
        }
        out.push_str("</available_skills>");
        out
    }

    #[must_use]
    pub fn augment_system_prompt(&self, base: &str) -> String {
        let mut s = base.trim().to_string();
        if !s.is_empty() {
            s.push_str("\n\n");
        }
        s.push_str(&self.summary_for_system_prompt());
        s.push_str(
            "\n\nIf a skill looks relevant to a user request, call load_skill(name) to fetch its instructions before acting. For any task that depends on the environment (OS, shell, compilers, repo layout), call load_skill(\"system-info\") first.",
        );
        s
    }

    pub fn load_skill_body(&self, name: &str) -> Result<String, SkillLoadError> {
        if !self.is_enabled(name) {
            return Err(SkillLoadError::Disabled(name.to_string()));
        }
        if name == SYSTEM_INFO_SKILL {
            return Ok(generate_system_info_body(self.workspace_root.as_deref()));
        }
        let skill =
            self.skills.get(name).ok_or_else(|| SkillLoadError::NotFound(name.to_string()))?;
        skill.body.clone().ok_or_else(|| SkillLoadError::NotFound(name.to_string()))
    }

    pub fn load_skill_reference(&self, name: &str, file: &str) -> Result<String, SkillLoadError> {
        if !self.is_enabled(name) {
            return Err(SkillLoadError::Disabled(name.to_string()));
        }
        let skill =
            self.skills.get(name).ok_or_else(|| SkillLoadError::NotFound(name.to_string()))?;
        if skill.path.as_os_str().is_empty() {
            return Err(SkillLoadError::NotFound(name.to_string()));
        }
        let candidate = skill.path.join(file);
        let canon_skill = fs::canonicalize(&skill.path).unwrap_or_else(|_| skill.path.clone());
        let canon_file = fs::canonicalize(&candidate)
            .map_err(|_| SkillLoadError::PathEscape(candidate.clone()))?;
        if !canon_file.starts_with(&canon_skill) {
            return Err(SkillLoadError::PathEscape(candidate));
        }
        fs::read_to_string(&canon_file).map_err(SkillLoadError::Io)
    }

    #[must_use]
    pub fn workspace_root(&self) -> Option<&Path> {
        self.workspace_root.as_deref()
    }
}

fn xml_escape(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '&' => "&amp;".to_string(),
            '<' => "&lt;".to_string(),
            '>' => "&gt;".to_string(),
            '"' => "&quot;".to_string(),
            _ => c.to_string(),
        })
        .collect()
}

#[must_use]
pub fn skill_path_changed(workspace_root: &Path, changed: &Path) -> bool {
    let prefix = workspace_root.join(".ide").join("skills");
    changed.starts_with(&prefix)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_xml_well_formed() {
        let reg = SkillRegistry::load(None, &SkillPersistence::default());
        let s = reg.summary_for_system_prompt();
        assert!(s.contains("<available_skills>"));
        assert!(s.contains("using-terminal"));
        assert!(s.contains("</available_skills>"));
    }
}
