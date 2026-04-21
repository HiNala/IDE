//! M20 smoke: path safety, registry, staged edits + rollback.

use std::fs;
use std::sync::{Arc, RwLock};

use editor_ai_tools::{
    path::canonical_under_workspace, registry::ToolRegistry, ToolConfig, WorkspaceTx,
};
use editor_workspace::{BufferManager, Workspace};
use tempfile::tempdir;

#[tokio::test]
async fn path_rejects_absolute_outside() {
    let tmp = tempdir().unwrap();
    let root = tmp.path().canonicalize().unwrap();
    #[cfg(unix)]
    {
        let r = canonical_under_workspace(&root, "/etc/passwd");
        assert!(matches!(r, Err(editor_ai_tools::ToolError::PathEscape(_))));
    }
    #[cfg(windows)]
    {
        let r = canonical_under_workspace(&root, "C:/Windows/system.ini");
        assert!(r.is_err());
    }
}

#[tokio::test]
async fn multi_tool_sequence_rollbacks() {
    let tmp = tempdir().unwrap();
    fs::create_dir_all(tmp.path().join("src")).unwrap();
    let p = tmp.path().join("src/a.rs");
    fs::write(&p, "line1\nline2\n").unwrap();

    let ws = Arc::new(Workspace::open(tmp.path()).unwrap());
    let buffers = Arc::new(RwLock::new(BufferManager::new()));
    let cfg = ToolConfig::default();
    let reg = ToolRegistry::new_default(&ws, &buffers, &cfg, None);

    let root = ws.root().to_path_buf();
    let mut tx = WorkspaceTx::new(root, Arc::clone(&buffers));

    let _ = reg
        .invoke(
            "insert_at",
            serde_json::json!({ "path": "src/a.rs", "line": 2, "content": "// hi\n" }),
            &mut tx,
            false,
        )
        .await
        .unwrap();

    let _ = reg
        .invoke(
            "append_to",
            serde_json::json!({ "path": "src/a.rs", "content": "tail" }),
            &mut tx,
            false,
        )
        .await
        .unwrap();

    assert_eq!(tx.pending_changes().len(), 2);
    tx.rollback_all();
    assert_eq!(tx.pending_changes().len(), 0);
}

/// Ten staging tools on **existing** files (staged edits are not visible to later tools until commit).
#[tokio::test]
async fn ten_staged_invokes_then_rollback() {
    let tmp = tempdir().unwrap();
    for i in 1..=10 {
        fs::write(tmp.path().join(format!("t{i}.txt")), "hello\n").unwrap();
    }

    let ws = Arc::new(Workspace::open(tmp.path()).unwrap());
    let buffers = Arc::new(RwLock::new(BufferManager::new()));
    let cfg = ToolConfig::default();
    let reg = ToolRegistry::new_default(&ws, &buffers, &cfg, None);

    let root = ws.root().to_path_buf();
    let mut tx = WorkspaceTx::new(root, Arc::clone(&buffers));

    macro_rules! run {
        ($name:expr, $json:expr) => {
            reg.invoke($name, $json, &mut tx, false)
                .await
                .unwrap_or_else(|e| panic!("tool {}: {e}", $name));
        };
    }

    for i in 1..=10 {
        run!(
            "append_to",
            serde_json::json!({ "path": format!("t{i}.txt"), "content": format!("-{i}") })
        );
    }

    assert_eq!(tx.pending_changes().len(), 10);
    tx.rollback_all();
    assert!(tx.pending_changes().is_empty());
}

#[test]
fn as_defs_sorted_and_nonempty() {
    let tmp = tempdir().unwrap();
    let ws = Arc::new(Workspace::open(tmp.path()).unwrap());
    let buffers = Arc::new(RwLock::new(BufferManager::new()));
    let reg = ToolRegistry::new_default(&ws, &buffers, &ToolConfig::default(), None);
    let defs = reg.as_defs();
    assert_eq!(defs.len(), 18);
    let mut names: Vec<_> = defs.iter().map(|d| d.name.as_str()).collect();
    let sorted = names.clone();
    names.sort();
    assert_eq!(names, sorted);
}

#[test]
fn skill_tools_fold_in_when_registry_provided() {
    use editor_skills::{SkillPersistence, SkillRegistry};

    let tmp = tempdir().unwrap();
    let ws = Arc::new(Workspace::open(tmp.path()).unwrap());
    let buffers = Arc::new(RwLock::new(BufferManager::new()));
    let skills = Arc::new(RwLock::new(SkillRegistry::load(None, &SkillPersistence::default())));
    let reg = ToolRegistry::new_default(&ws, &buffers, &ToolConfig::default(), Some(skills));
    let defs = reg.as_defs();
    assert_eq!(defs.len(), 21);
    let names: Vec<_> = defs.iter().map(|d| d.name.as_str()).collect();
    assert!(names.contains(&"load_skill"));
    assert!(names.contains(&"list_skills"));
    assert!(names.contains(&"load_skill_reference"));
}
