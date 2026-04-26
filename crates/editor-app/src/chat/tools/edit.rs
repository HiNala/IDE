//! File editing tools: edit_lines, insert_at, append_to, replace_in_file,
//! create_file, delete_file, move_file.

use std::io::Write as _;
use std::path::Path;

use super::resolve_path;

pub(super) fn tool_edit_lines(input: &serde_json::Value, root: &Path) -> (String, bool) {
    let path = match resolve_path(input, "path", root) {
        Err(e) => return (e, true),
        Ok(p) => p,
    };
    let start_line = match input["start_line"].as_u64() {
        Some(n) if n >= 1 => n as usize,
        _ => return ("'start_line' must be a positive integer".into(), true),
    };
    let end_line = match input["end_line"].as_u64() {
        Some(n) if n >= start_line as u64 => n as usize,
        _ => return ("'end_line' must be >= start_line".into(), true),
    };
    let new_content = match input["new_content"].as_str() {
        Some(s) => s,
        None => return ("Missing required field 'new_content'".into(), true),
    };
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) => return (format!("Cannot read {}: {e}", path.display()), true),
    };
    let mut lines: Vec<&str> = content.lines().collect();
    if end_line > lines.len() {
        return (format!("end_line {end_line} > file length {}", lines.len()), true);
    }
    let new_lines: Vec<&str> = new_content.lines().collect();
    lines.splice((start_line - 1)..end_line, new_lines);
    let new_file = lines.join("\n") + if content.ends_with('\n') { "\n" } else { "" };
    match std::fs::write(&path, &new_file) {
        Ok(()) => (format!("Replaced lines {start_line}–{end_line} in {}", path.display()), false),
        Err(e) => (format!("Cannot write {}: {e}", path.display()), true),
    }
}

pub(super) fn tool_insert_at(input: &serde_json::Value, root: &Path) -> (String, bool) {
    let path = match resolve_path(input, "path", root) {
        Err(e) => return (e, true),
        Ok(p) => p,
    };
    let line_number = match input["line_number"].as_u64() {
        Some(n) if n >= 1 => n as usize,
        _ => return ("'line_number' must be a positive integer".into(), true),
    };
    let content_str = match input["content"].as_str() {
        Some(s) => s,
        None => return ("Missing required field 'content'".into(), true),
    };
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) => return (format!("Cannot read {}: {e}", path.display()), true),
    };
    let mut lines: Vec<&str> = content.lines().collect();
    let insert_at = (line_number - 1).min(lines.len());
    for (i, l) in content_str.lines().enumerate() {
        lines.insert(insert_at + i, l);
    }
    let new_file = lines.join("\n") + if content.ends_with('\n') { "\n" } else { "" };
    match std::fs::write(&path, &new_file) {
        Ok(()) => (
            format!(
                "Inserted {} line(s) at line {line_number} in {}",
                content_str.lines().count(),
                path.display()
            ),
            false,
        ),
        Err(e) => (format!("Cannot write {}: {e}", path.display()), true),
    }
}

pub(super) fn tool_append_to(input: &serde_json::Value, root: &Path) -> (String, bool) {
    let path = match resolve_path(input, "path", root) {
        Err(e) => return (e, true),
        Ok(p) => p,
    };
    let content_str = match input["content"].as_str() {
        Some(s) => s,
        None => return ("Missing required field 'content'".into(), true),
    };
    match std::fs::OpenOptions::new().append(true).create(true).open(&path) {
        Ok(mut f) => match f.write_all(content_str.as_bytes()) {
            Ok(()) => (format!("Appended {} bytes to {}", content_str.len(), path.display()), false),
            Err(e) => (format!("Write error on {}: {e}", path.display()), true),
        },
        Err(e) => (format!("Cannot open {} for append: {e}", path.display()), true),
    }
}

pub(super) fn tool_replace_in_file(input: &serde_json::Value, root: &Path) -> (String, bool) {
    let path = match resolve_path(input, "path", root) {
        Err(e) => return (e, true),
        Ok(p) => p,
    };
    let old_text = match input["old_text"].as_str() {
        Some(s) => s,
        None => return ("Missing required field 'old_text'".into(), true),
    };
    let new_text = match input["new_text"].as_str() {
        Some(s) => s,
        None => return ("Missing required field 'new_text'".into(), true),
    };
    // occurrence: 0 = replace all, N = replace Nth (1-based). Default = 1.
    let occurrence = input["occurrence"].as_u64().unwrap_or(1) as usize;

    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) => return (format!("Cannot read {}: {e}", path.display()), true),
    };
    if !content.contains(old_text) {
        return (format!("'old_text' not found in {}", path.display()), true);
    }

    let new_content = if occurrence == 0 {
        content.replace(old_text, new_text)
    } else {
        let mut result = String::with_capacity(content.len());
        let mut remaining = content.as_str();
        let mut found = 0usize;
        let mut replaced = false;
        while let Some(pos) = remaining.find(old_text) {
            found += 1;
            result.push_str(&remaining[..pos]);
            if found == occurrence {
                result.push_str(new_text);
                replaced = true;
                remaining = &remaining[pos + old_text.len()..];
                break;
            } else {
                result.push_str(old_text);
                remaining = &remaining[pos + old_text.len()..];
            }
        }
        result.push_str(remaining);
        if !replaced {
            return (
                format!(
                    "Occurrence {occurrence} not found (file has {found} occurrence(s))"
                ),
                true,
            );
        }
        result
    };

    match std::fs::write(&path, &new_content) {
        Ok(()) => (
            format!(
                "Replaced in {} (occurrence {})",
                path.display(),
                if occurrence == 0 { "all".to_string() } else { occurrence.to_string() }
            ),
            false,
        ),
        Err(e) => (format!("Cannot write {}: {e}", path.display()), true),
    }
}

pub(super) fn tool_create_file(input: &serde_json::Value, root: &Path) -> (String, bool) {
    let path = match resolve_path(input, "path", root) {
        Err(e) => return (e, true),
        Ok(p) => p,
    };
    if path.exists() {
        return (format!("{} already exists", path.display()), true);
    }
    let content_str = input["content"].as_str().unwrap_or("");
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    match std::fs::write(&path, content_str) {
        Ok(()) => (format!("Created {} ({} bytes)", path.display(), content_str.len()), false),
        Err(e) => (format!("Cannot create {}: {e}", path.display()), true),
    }
}

pub(super) fn tool_delete_file(input: &serde_json::Value, root: &Path) -> (String, bool) {
    let path = match resolve_path(input, "path", root) {
        Err(e) => return (e, true),
        Ok(p) => p,
    };
    match std::fs::remove_file(&path) {
        Ok(()) => (format!("Deleted {}", path.display()), false),
        Err(e) => (format!("Cannot delete {}: {e}", path.display()), true),
    }
}

pub(super) fn tool_move_file(input: &serde_json::Value, root: &Path) -> (String, bool) {
    let from = match resolve_path(input, "from", root) {
        Err(e) => return (e, true),
        Ok(p) => p,
    };
    let to = match resolve_path(input, "to", root) {
        Err(e) => return (e, true),
        Ok(p) => p,
    };
    if to.exists() {
        return (format!("Destination {} already exists", to.display()), true);
    }
    if let Some(parent) = to.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    match std::fs::rename(&from, &to) {
        Ok(()) => (format!("Moved {} → {}", from.display(), to.display()), false),
        Err(e) => (format!("Cannot move: {e}"), true),
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn tmp() -> TempDir { tempfile::tempdir().unwrap() }
    fn j(s: &str) -> serde_json::Value { serde_json::from_str(s).unwrap() }

    #[test]
    fn edit_lines_replaces_range() {
        let d = tmp();
        std::fs::write(d.path().join("f.txt"), "a\nb\nc\nd\n").unwrap();
        let (out, err) = tool_edit_lines(
            &j(r#"{"path":"f.txt","start_line":2,"end_line":3,"new_content":"X\nY"}"#),
            d.path(),
        );
        assert!(!err, "{out}");
        assert_eq!(std::fs::read_to_string(d.path().join("f.txt")).unwrap(), "a\nX\nY\nd\n");
    }

    #[test]
    fn replace_first_occurrence() {
        let d = tmp();
        std::fs::write(d.path().join("code.rs"), "foo foo foo\n").unwrap();
        let (out, err) = tool_replace_in_file(
            &j(r#"{"path":"code.rs","old_text":"foo","new_text":"bar","occurrence":1}"#),
            d.path(),
        );
        assert!(!err, "{out}");
        assert_eq!(std::fs::read_to_string(d.path().join("code.rs")).unwrap(), "bar foo foo\n");
    }

    #[test]
    fn replace_all_occurrences() {
        let d = tmp();
        std::fs::write(d.path().join("code.rs"), "foo foo foo\n").unwrap();
        tool_replace_in_file(
            &j(r#"{"path":"code.rs","old_text":"foo","new_text":"baz","occurrence":0}"#),
            d.path(),
        );
        assert_eq!(std::fs::read_to_string(d.path().join("code.rs")).unwrap(), "baz baz baz\n");
    }

    #[test]
    fn create_then_append_then_move() {
        let d = tmp();
        let (out1, err1) = tool_create_file(&j(r#"{"path":"app/index.ts","content":"export {};\n"}"#), d.path());
        assert!(!err1, "{out1}");

        let (out2, err2) = tool_append_to(&j(r#"{"path":"app/index.ts","content":"// note\n"}"#), d.path());
        assert!(!err2, "{out2}");

        let content = std::fs::read_to_string(d.path().join("app/index.ts")).unwrap();
        assert!(content.contains("// note"));

        let (out3, err3) = tool_move_file(&j(r#"{"from":"app/index.ts","to":"app/entry.ts"}"#), d.path());
        assert!(!err3, "{out3}");
        assert!(d.path().join("app/entry.ts").exists());
    }
}
