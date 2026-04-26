//! Task-list tools: list_tasks, add_task, complete_task, update_task.
//! Task file lives at `.ide/tasks.md` in the workspace root.

use std::path::Path;

use editor_metadata::{tasks_path, Task, TaskList, TaskStatus};

fn new_id(seed: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    seed.hash(&mut h);
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos()
        .hash(&mut h);
    format!("{:016x}", h.finish()).chars().take(8).collect()
}

fn load(root: &Path) -> (TaskList, Option<String>) {
    let p = tasks_path(root);
    if !p.exists() {
        return (TaskList::empty(), None);
    }
    match std::fs::read_to_string(&p) {
        Ok(raw) => (TaskList::parse(&raw).unwrap_or_else(|_| TaskList::empty()), None),
        Err(e) => (TaskList::empty(), Some(format!("Cannot read tasks.md: {e}"))),
    }
}

fn save(root: &Path, list: &TaskList) -> Option<String> {
    let p = tasks_path(root);
    if let Some(parent) = p.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    std::fs::write(&p, list.to_markdown()).err().map(|e| format!("Cannot write tasks.md: {e}"))
}

pub(super) fn tool_list_tasks(workspace_root: &Path) -> (String, bool) {
    let (list, err) = load(workspace_root);
    if let Some(e) = err {
        return (e, true);
    }
    if list.entries.is_empty() {
        return ("No tasks found. Use add_task to create one.".into(), false);
    }
    match serde_json::to_string_pretty(&list.entries) {
        Ok(json) => (json, false),
        Err(e) => (format!("Serialize error: {e}"), true),
    }
}

pub(super) fn tool_add_task(input: &serde_json::Value, workspace_root: &Path) -> (String, bool) {
    let summary = match input["summary"].as_str() {
        Some(s) => s,
        None => return ("Missing required field 'summary'".into(), true),
    };
    let notes = input["notes"].as_str().map(|s| s.to_string());
    let (mut list, err) = load(workspace_root);
    if let Some(e) = err {
        return (e, true);
    }
    let id = new_id(summary);
    list.entries.push(Task {
        id: id.clone(),
        summary: summary.to_string(),
        status: TaskStatus::Open,
        notes,
    });
    if let Some(e) = save(workspace_root, &list) {
        return (e, true);
    }
    (format!("Added task `{id}` — {summary}"), false)
}

pub(super) fn tool_complete_task(
    input: &serde_json::Value,
    workspace_root: &Path,
) -> (String, bool) {
    let id = match input["id"].as_str() {
        Some(s) => s,
        None => return ("Missing required field 'id'".into(), true),
    };
    let (mut list, err) = load(workspace_root);
    if let Some(e) = err {
        return (e, true);
    }
    match list.entries.iter().position(|t| t.id == id) {
        None => (format!("Task `{id}` not found"), true),
        Some(idx) => {
            list.entries[idx].status = TaskStatus::Done;
            let summary = list.entries[idx].summary.clone();
            if let Some(e) = save(workspace_root, &list) {
                return (e, true);
            }
            (format!("Completed task `{id}` — {summary}"), false)
        }
    }
}

pub(super) fn tool_update_task(
    input: &serde_json::Value,
    workspace_root: &Path,
) -> (String, bool) {
    let id = match input["id"].as_str() {
        Some(s) => s,
        None => return ("Missing required field 'id'".into(), true),
    };
    let (mut list, err) = load(workspace_root);
    if let Some(e) = err {
        return (e, true);
    }
    match list.entries.iter().position(|t| t.id == id) {
        None => (format!("Task `{id}` not found"), true),
        Some(idx) => {
            if let Some(status_str) = input["status"].as_str() {
                list.entries[idx].status = match status_str {
                    "open" => TaskStatus::Open,
                    "in_progress" => TaskStatus::InProgress,
                    "done" => TaskStatus::Done,
                    "cancelled" => TaskStatus::Cancelled,
                    other => return (format!("Unknown status '{other}'"), true),
                };
            }
            if let Some(notes) = input["notes"].as_str() {
                list.entries[idx].notes = Some(notes.to_string());
            }
            let summary = list.entries[idx].summary.clone();
            if let Some(e) = save(workspace_root, &list) {
                return (e, true);
            }
            (format!("Updated task `{id}` — {summary}"), false)
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn tmp() -> TempDir { tempfile::tempdir().unwrap() }

    #[test]
    fn empty_workspace_has_no_tasks() {
        let d = tmp();
        let (out, err) = tool_list_tasks(d.path());
        assert!(!err, "{out}");
        assert!(out.contains("No tasks"), "{out}");
    }

    #[test]
    fn add_then_list() {
        let d = tmp();
        let (add, aerr) = tool_add_task(
            &serde_json::json!({"summary": "Write auth tests"}),
            d.path(),
        );
        assert!(!aerr, "{add}");
        assert!(add.contains("Write auth tests"), "{add}");

        let (list, lerr) = tool_list_tasks(d.path());
        assert!(!lerr, "{list}");
        assert!(list.contains("Write auth tests"), "{list}");
    }

    #[test]
    fn complete_task_marks_done() {
        let d = tmp();
        tool_add_task(&serde_json::json!({"summary":"Fix login bug"}), d.path());
        let (list, _) = tool_list_tasks(d.path());
        let v: serde_json::Value = serde_json::from_str(&list).unwrap();
        let id = v[0]["id"].as_str().unwrap().to_string();

        let (out, err) = tool_complete_task(&serde_json::json!({"id": id}), d.path());
        assert!(!err, "{out}");

        let (list2, _) = tool_list_tasks(d.path());
        let v2: serde_json::Value = serde_json::from_str(&list2).unwrap();
        assert_eq!(v2[0]["status"], "done", "{list2}");
    }

    #[test]
    fn update_task_status_and_notes() {
        let d = tmp();
        tool_add_task(&serde_json::json!({"summary":"Refactor auth"}), d.path());
        let (list, _) = tool_list_tasks(d.path());
        let v: serde_json::Value = serde_json::from_str(&list).unwrap();
        let id = v[0]["id"].as_str().unwrap().to_string();

        let (out, err) = tool_update_task(
            &serde_json::json!({"id": id, "status": "in_progress", "notes": "Blocked on PR #42"}),
            d.path(),
        );
        assert!(!err, "{out}");

        let (list2, _) = tool_list_tasks(d.path());
        let v2: serde_json::Value = serde_json::from_str(&list2).unwrap();
        assert_eq!(v2[0]["status"], "in_progress");
        assert!(v2[0]["notes"].as_str().unwrap().contains("PR #42"));
    }

    #[test]
    fn complete_nonexistent_returns_error() {
        let d = tmp();
        let (out, err) = tool_complete_task(&serde_json::json!({"id":"ghost"}), d.path());
        assert!(err, "{out}");
        assert!(out.contains("not found"), "{out}");
    }
}
