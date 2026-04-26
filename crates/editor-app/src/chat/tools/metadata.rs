//! Metadata sidecar tools: read_metadata, write_metadata_note.

use std::path::Path;

use editor_metadata::{blank_sidecar, write_to_markdown, MetadataStore};

use super::resolve_path;

pub(super) fn tool_read_metadata(
    input: &serde_json::Value,
    workspace_root: &Path,
    metadata_store: Option<&MetadataStore>,
) -> (String, bool) {
    let path = match resolve_path(input, "path", workspace_root) {
        Err(e) => return (e, true),
        Ok(p) => p,
    };
    let store_owned;
    let store = if let Some(s) = metadata_store {
        s
    } else {
        store_owned = MetadataStore::new(workspace_root.to_path_buf());
        &store_owned
    };
    match store.load(&path) {
        Err(e) => (format!("Metadata error: {e}"), true),
        Ok(None) => (
            format!(
                "No sidecar found for {}. Use write_metadata_note to create one.",
                path.display()
            ),
            false,
        ),
        Ok(Some(sidecar)) => match write_to_markdown(&sidecar) {
            Ok(md) => (md, false),
            Err(e) => (format!("Serialize error: {e}"), true),
        },
    }
}

pub(super) fn tool_write_metadata_note(
    input: &serde_json::Value,
    workspace_root: &Path,
    metadata_store: Option<&MetadataStore>,
) -> (String, bool) {
    let path = match resolve_path(input, "path", workspace_root) {
        Err(e) => return (e, true),
        Ok(p) => p,
    };
    let note = match input["note"].as_str() {
        Some(s) => s,
        None => return ("Missing required field 'note'".into(), true),
    };
    let store_owned;
    let store = if let Some(s) = metadata_store {
        s
    } else {
        store_owned = MetadataStore::new(workspace_root.to_path_buf());
        &store_owned
    };
    let rel_path = path.strip_prefix(workspace_root).unwrap_or(&path).to_path_buf();
    let mut sidecar = store.load(&path).unwrap_or(None).unwrap_or_else(|| {
        let content = std::fs::read_to_string(&path).unwrap_or_default();
        blank_sidecar(&rel_path, &content, "claude-opus-4-7")
    });
    let ts = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ");
    let note_with_ts = format!("[{ts}] {note}");
    if sidecar.body.notes.is_empty() {
        sidecar.body.notes = note_with_ts;
    } else {
        sidecar.body.notes.push('\n');
        sidecar.body.notes.push_str(&note_with_ts);
    }
    sidecar.frontmatter.last_updated = chrono::Utc::now();
    match store.save(&sidecar) {
        Ok(()) => (format!("Wrote metadata note for {}", path.display()), false),
        Err(e) => (format!("Cannot save sidecar: {e}"), true),
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn tmp() -> TempDir { tempfile::tempdir().unwrap() }

    fn make_file(d: &TempDir, rel: &str, content: &str) {
        let p = d.path().join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, content).unwrap();
    }

    #[test]
    fn read_missing_returns_hint() {
        let d = tmp();
        make_file(&d, "src/main.rs", "fn main() {}");
        let (out, err) = tool_read_metadata(
            &serde_json::json!({"path": "src/main.rs"}),
            d.path(),
            None,
        );
        assert!(!err, "{out}");
        assert!(out.contains("No sidecar"), "{out}");
    }

    #[test]
    fn write_then_read_round_trip() {
        let d = tmp();
        make_file(&d, "src/lib.rs", "pub fn add(a: i32, b: i32) -> i32 { a + b }");

        let (w_out, w_err) = tool_write_metadata_note(
            &serde_json::json!({"path": "src/lib.rs", "note": "pure addition helper"}),
            d.path(),
            None,
        );
        assert!(!w_err, "{w_out}");

        let sidecar_path = d.path().join(".ide/meta/src/lib.rs.md");
        assert!(sidecar_path.exists(), "sidecar should be created");

        let (r_out, r_err) = tool_read_metadata(
            &serde_json::json!({"path": "src/lib.rs"}),
            d.path(),
            None,
        );
        assert!(!r_err, "{r_out}");
        assert!(r_out.contains("pure addition helper"), "{r_out}");
    }

    #[test]
    fn multiple_notes_accumulate() {
        let d = tmp();
        make_file(&d, "a.ts", "const x = 1;");
        tool_write_metadata_note(&serde_json::json!({"path":"a.ts","note":"first"}), d.path(), None);
        tool_write_metadata_note(&serde_json::json!({"path":"a.ts","note":"second"}), d.path(), None);
        let (out, err) = tool_read_metadata(&serde_json::json!({"path":"a.ts"}), d.path(), None);
        assert!(!err, "{out}");
        assert!(out.contains("first") && out.contains("second"), "{out}");
    }
}
