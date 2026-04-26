//! Skills tools: load_skill, list_skills.

use editor_skills::SkillRegistry;

pub(super) fn tool_load_skill(
    input: &serde_json::Value,
    skill_registry: Option<&SkillRegistry>,
) -> (String, bool) {
    let name = match input["name"].as_str() {
        Some(s) => s,
        None => return ("Missing required field 'name'".into(), true),
    };
    let Some(registry) = skill_registry else {
        return ("Skill registry not available".into(), true);
    };
    match registry.load_skill_body(name) {
        Ok(body) => (body, false),
        Err(e) => (format!("Cannot load skill '{name}': {e}"), true),
    }
}

pub(super) fn tool_list_skills(skill_registry: Option<&SkillRegistry>) -> (String, bool) {
    let Some(registry) = skill_registry else {
        return ("Skill registry not available".into(), true);
    };
    let skills: Vec<_> = registry
        .list()
        .into_iter()
        .map(|s| serde_json::json!({ "name": s.name, "description": s.description }))
        .collect();
    match serde_json::to_string_pretty(&skills) {
        Ok(json) => (json, false),
        Err(e) => (format!("Serialize error: {e}"), true),
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_skills_no_registry_returns_error() {
        let (out, err) = tool_list_skills(None);
        assert!(err, "{out}");
    }

    #[test]
    fn load_skill_no_registry_returns_error() {
        let (out, err) = tool_load_skill(&serde_json::json!({"name":"system-info"}), None);
        assert!(err, "{out}");
    }

    #[test]
    fn list_skills_with_default_registry() {
        let reg = SkillRegistry::load(None, &Default::default());
        let (out, err) = tool_list_skills(Some(&reg));
        assert!(!err, "{out}");
        // The default registry includes built-in skills.
        let v: serde_json::Value = serde_json::from_str(&out).expect("valid json");
        assert!(v.as_array().is_some_and(|a| !a.is_empty()), "expected skills: {out}");
    }

    #[test]
    fn system_info_skill_loads() {
        let reg = SkillRegistry::load(None, &Default::default());
        let (out, err) = tool_load_skill(&serde_json::json!({"name":"system-info"}), Some(&reg));
        assert!(!err, "{out}");
        assert!(!out.is_empty(), "system-info body should not be empty");
    }
}
