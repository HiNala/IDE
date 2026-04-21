# Authoring agent skills

This IDE uses **Anthropic-style** skill folders: each skill is a directory containing `SKILL.md` with YAML frontmatter and a markdown body.

## Frontmatter (required)

- `name` — stable id (matches directory name in examples).
- `description` — **pushy**: say when to load this skill; models under-trigger if descriptions are vague.

Optional: `allowed_tools`, `disable_model_invocation`, `compatibility`.

## Discovery order

1. Built-in skills (embedded in the binary).
2. User: `%APPDATA%\ide\skills` (Windows), `~/.config/ide/skills` (Linux), `~/Library/Application Support/ide/skills` (macOS).
3. Extra directories from settings (future).
4. Workspace: `<root>/.ide/skills/<name>/SKILL.md`.

Later entries **override** earlier ones on name conflict.

## Progressive disclosure

Only **name + description** go into the system prompt. The agent calls `load_skill(name)` to read the body. Use `load_skill_reference` for sibling files; paths cannot escape the skill directory.

## Persistence (`state.json`)

- `skills_disabled`: string array of skill ids to hide from the catalog and block in tools.
- `extra_skill_dirs`: additional roots to scan for `*/SKILL.md`.

Build a registry with `SkillRegistry::load(workspace_root, &persisted.skill_persistence())`.

## `system-info`

Dynamic body (cached ~5 minutes); use when behavior depends on OS, shell, or tool versions.

*Last updated: M27.*
