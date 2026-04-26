//! Tool dispatch and shared helpers for the AI tool execution loop.

mod edit;
mod fs;
mod metadata;
mod shell;
mod skills;
mod tasks;

use std::path::Path;

use editor_metadata::MetadataStore;
use editor_skills::SkillRegistry;

// ── Path helper (shared across tool modules) ──────────────────────────────────

pub(super) fn resolve_path(
    input: &serde_json::Value,
    key: &str,
    root: &Path,
) -> Result<std::path::PathBuf, String> {
    let s = input[key]
        .as_str()
        .ok_or_else(|| format!("Missing required field '{key}'"))?;
    let p = Path::new(s);
    Ok(if p.is_absolute() { p.to_path_buf() } else { root.join(p) })
}

// ── Main dispatch ─────────────────────────────────────────────────────────────

/// Execute a named tool synchronously and return `(result_text, is_error)`.
///
/// All tools operate on `workspace_root` as the path base. Write tools go
/// directly to disk; the IDE's file-watcher detects and surfaces changes.
pub(crate) fn execute_tool(
    name: &str,
    input_json: &str,
    workspace_root: &Path,
    skill_registry: Option<&SkillRegistry>,
    metadata_store: Option<&MetadataStore>,
) -> (String, bool) {
    let input: serde_json::Value = match serde_json::from_str(input_json) {
        Ok(v) => v,
        Err(e) => return (format!("Invalid tool input JSON: {e}"), true),
    };

    match name {
        // Filesystem exploration
        "read_file" => fs::tool_read_file(&input, workspace_root),
        "list_directory" => fs::tool_list_directory(&input, workspace_root),
        "find_files" => fs::tool_find_files(&input, workspace_root),
        "grep" => fs::tool_grep(&input, workspace_root),
        // File edits
        "edit_lines" => edit::tool_edit_lines(&input, workspace_root),
        "insert_at" => edit::tool_insert_at(&input, workspace_root),
        "append_to" => edit::tool_append_to(&input, workspace_root),
        "replace_in_file" => edit::tool_replace_in_file(&input, workspace_root),
        "create_file" => edit::tool_create_file(&input, workspace_root),
        "delete_file" => edit::tool_delete_file(&input, workspace_root),
        "move_file" => edit::tool_move_file(&input, workspace_root),
        // Shell
        "run_shell" => shell::tool_run_shell(&input, workspace_root),
        // Metadata sidecar
        "read_metadata" => metadata::tool_read_metadata(&input, workspace_root, metadata_store),
        "write_metadata_note" => {
            metadata::tool_write_metadata_note(&input, workspace_root, metadata_store)
        }
        // Task list
        "list_tasks" => tasks::tool_list_tasks(workspace_root),
        "add_task" => tasks::tool_add_task(&input, workspace_root),
        "complete_task" => tasks::tool_complete_task(&input, workspace_root),
        "update_task" => tasks::tool_update_task(&input, workspace_root),
        // Skills
        "load_skill" => skills::tool_load_skill(&input, skill_registry),
        "list_skills" => skills::tool_list_skills(skill_registry),
        other => (format!("Unknown tool: {other}"), true),
    }
}

// ── Integration tests ─────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn tmp() -> TempDir { tempfile::tempdir().unwrap() }

    const KNOWN_TOOLS: &[(&str, &str)] = &[
        ("read_file",           r#"{"path":"dummy.txt"}"#),
        ("list_directory",      r#"{"path":"."}"#),
        ("find_files",          r#"{"pattern":"*.txt"}"#),
        ("grep",                r#"{"pattern":"hello"}"#),
        ("list_tasks",          r#"{}"#),
        ("list_skills",         r#"{}"#),
        ("run_shell",           r#"{"command":"echo x"}"#),
    ];

    #[test]
    fn all_known_tools_dispatch_without_unknown_error() {
        let d = tmp();
        std::fs::write(d.path().join("dummy.txt"), "hello").unwrap();
        for (name, input) in KNOWN_TOOLS {
            let (out, _) = execute_tool(name, input, d.path(), None, None);
            assert!(
                !out.starts_with("Unknown tool:"),
                "tool '{name}' was not dispatched: {out}"
            );
        }
    }

    #[test]
    fn unknown_tool_returns_error_message() {
        let d = tmp();
        let (out, err) = execute_tool("nonexistent", "{}", d.path(), None, None);
        assert!(err);
        assert!(out.starts_with("Unknown tool:"), "{out}");
    }

    #[test]
    fn scaffold_sequence() {
        let d = tmp();

        let (_, e1) = execute_tool(
            "create_file",
            r#"{"path":"app/index.ts","content":"export {};\n"}"#,
            d.path(), None, None,
        );
        assert!(!e1);

        let (_, e2) = execute_tool(
            "append_to",
            r#"{"path":"app/index.ts","content":"// auto\n"}"#,
            d.path(), None, None,
        );
        assert!(!e2);

        let (read_out, e3) = execute_tool(
            "read_file",
            r#"{"path":"app/index.ts"}"#,
            d.path(), None, None,
        );
        assert!(!e3);
        assert!(read_out.contains("auto"), "{read_out}");

        let (dir_out, e4) = execute_tool(
            "list_directory",
            r#"{"path":"app"}"#,
            d.path(), None, None,
        );
        assert!(!e4);
        assert!(dir_out.contains("index.ts"), "{dir_out}");
    }
}
