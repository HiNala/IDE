//! File-system exploration tools: read_file, list_directory, find_files, grep.

use std::io::{BufRead, BufReader};
use std::path::Path;

use super::resolve_path;

pub(super) fn tool_read_file(input: &serde_json::Value, root: &Path) -> (String, bool) {
    match resolve_path(input, "path", root) {
        Err(e) => (e, true),
        Ok(path) => match std::fs::read_to_string(&path) {
            Ok(content) => {
                let lines = content.lines().count();
                (format!("File: {} ({lines} lines)\n---\n{content}", path.display()), false)
            }
            Err(e) => (format!("Cannot read {}: {e}", path.display()), true),
        },
    }
}

pub(super) fn tool_list_directory(input: &serde_json::Value, root: &Path) -> (String, bool) {
    let recursive = input["recursive"].as_bool().unwrap_or(false);
    match resolve_path(input, "path", root) {
        Err(e) => (e, true),
        Ok(path) => {
            if !path.is_dir() {
                return (format!("{} is not a directory", path.display()), true);
            }
            let mut entries: Vec<String> = Vec::new();
            if recursive {
                collect_entries_recursive(&path, &path, &mut entries, 0, 5);
            } else {
                match std::fs::read_dir(&path) {
                    Err(e) => return (format!("Cannot list {}: {e}", path.display()), true),
                    Ok(rd) => {
                        let mut names: Vec<String> = rd
                            .filter_map(|e| e.ok())
                            .map(|e| {
                                let is_dir = e.file_type().map(|t| t.is_dir()).unwrap_or(false);
                                let n = e.file_name().to_string_lossy().to_string();
                                if is_dir { format!("{n}/") } else { n }
                            })
                            .collect();
                        names.sort();
                        entries = names;
                    }
                }
            }
            if entries.is_empty() {
                return (format!("{} is empty", path.display()), false);
            }
            (entries.join("\n"), false)
        }
    }
}

fn collect_entries_recursive(
    root: &Path,
    dir: &Path,
    out: &mut Vec<String>,
    depth: usize,
    max_depth: usize,
) {
    if depth > max_depth {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut entries: Vec<_> = rd.filter_map(|e| e.ok()).collect();
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
        let name = entry.file_name().to_string_lossy().to_string();
        if is_dir && (name == "node_modules" || name == ".git" || name == "target") {
            out.push(format!("{name}/ (skipped)"));
            continue;
        }
        let rel = entry
            .path()
            .strip_prefix(root)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| name.clone());
        if is_dir {
            out.push(format!("{rel}/"));
            collect_entries_recursive(root, &entry.path(), out, depth + 1, max_depth);
        } else {
            out.push(rel);
        }
    }
}

pub(super) fn tool_find_files(input: &serde_json::Value, root: &Path) -> (String, bool) {
    let pattern = match input["pattern"].as_str() {
        Some(p) => p,
        None => return ("Missing required field 'pattern'".into(), true),
    };
    let search_root = if let Some(p) = input["path"].as_str() {
        root.join(p)
    } else {
        root.to_path_buf()
    };
    let glob_pat = if search_root != *root && !pattern.starts_with("**/") {
        format!("**/{pattern}")
    } else {
        pattern.to_string()
    };
    let mut matches: Vec<String> = Vec::new();
    walk_and_match(&search_root, &search_root, &glob_pat, &mut matches, 0, 8);
    matches.sort();
    matches.truncate(200);
    if matches.is_empty() {
        (format!("No files match '{pattern}'"), false)
    } else {
        (matches.join("\n"), false)
    }
}

fn walk_and_match(
    root: &Path,
    dir: &Path,
    pattern: &str,
    out: &mut Vec<String>,
    depth: usize,
    max_depth: usize,
) {
    if depth > max_depth {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for entry in rd.filter_map(|e| e.ok()) {
        let path = entry.path();
        let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') || name == "node_modules" || name == "target" {
            continue;
        }
        if is_dir {
            walk_and_match(root, &path, pattern, out, depth + 1, max_depth);
        } else {
            let rel = path
                .strip_prefix(root)
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| name.clone());
            if glob_match(pattern, &rel) || glob_match(pattern, &name) {
                out.push(rel);
            }
        }
    }
}

// ── Glob matching ─────────────────────────────────────────────────────────────

pub(super) fn glob_match(pattern: &str, s: &str) -> bool {
    let pat_parts: Vec<&str> = pattern.split('/').collect();
    let s_parts: Vec<&str> = s.split('/').collect();
    glob_match_parts(&pat_parts, &s_parts)
}

fn glob_match_parts(pat: &[&str], s: &[&str]) -> bool {
    if pat.is_empty() {
        return s.is_empty();
    }
    if pat[0] == "**" {
        if pat.len() == 1 {
            return true;
        }
        for i in 0..=s.len() {
            if glob_match_parts(&pat[1..], &s[i..]) {
                return true;
            }
        }
        return false;
    }
    if s.is_empty() {
        return false;
    }
    if wildcard_match(pat[0], s[0]) {
        return glob_match_parts(&pat[1..], &s[1..]);
    }
    false
}

pub(super) fn wildcard_match(pattern: &str, s: &str) -> bool {
    let (mut pi, mut si) = (0usize, 0usize);
    let pb = pattern.as_bytes();
    let sb = s.as_bytes();
    let (mut star_pi, mut star_si) = (usize::MAX, usize::MAX);
    while si < sb.len() {
        if pi < pb.len() && (pb[pi] == b'?' || pb[pi] == sb[si]) {
            pi += 1;
            si += 1;
        } else if pi < pb.len() && pb[pi] == b'*' {
            star_pi = pi;
            star_si = si;
            pi += 1;
        } else if star_pi != usize::MAX {
            star_si += 1;
            si = star_si;
            pi = star_pi + 1;
        } else {
            return false;
        }
    }
    while pi < pb.len() && pb[pi] == b'*' {
        pi += 1;
    }
    pi == pb.len()
}

// ── Grep ─────────────────────────────────────────────────────────────────────

pub(super) fn tool_grep(input: &serde_json::Value, root: &Path) -> (String, bool) {
    let pattern_str = match input["pattern"].as_str() {
        Some(p) => p,
        None => return ("Missing required field 'pattern'".into(), true),
    };
    let search_root = if let Some(p) = input["path"].as_str() {
        root.join(p)
    } else {
        root.to_path_buf()
    };
    let file_pattern = input["file_pattern"].as_str().unwrap_or("*");
    let context = input["context_lines"].as_u64().unwrap_or(2) as usize;
    let mut results: Vec<String> = Vec::new();
    let mut file_count = 0usize;
    grep_dir(
        &search_root,
        &search_root,
        pattern_str,
        file_pattern,
        context,
        &mut results,
        &mut file_count,
        0,
        6,
    );
    results.truncate(500);
    if results.is_empty() {
        (format!("No matches for '{pattern_str}'"), false)
    } else {
        (results.join("\n"), false)
    }
}

#[allow(clippy::too_many_arguments)]
fn grep_dir(
    root: &Path,
    dir: &Path,
    pattern: &str,
    file_pattern: &str,
    context: usize,
    out: &mut Vec<String>,
    file_count: &mut usize,
    depth: usize,
    max_depth: usize,
) {
    if depth > max_depth || *file_count > 100 {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut entries: Vec<_> = rd.filter_map(|e| e.ok()).collect();
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        let path = entry.path();
        let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') || name == "node_modules" || name == "target" {
            continue;
        }
        if is_dir {
            grep_dir(root, &path, pattern, file_pattern, context, out, file_count, depth + 1, max_depth);
        } else if wildcard_match(file_pattern, &name) {
            let Ok(f) = std::fs::File::open(&path) else { continue };
            *file_count += 1;
            let rel = path
                .strip_prefix(root)
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| name.clone());
            let reader = BufReader::new(f);
            let lines: Vec<String> = reader.lines().filter_map(|l| l.ok()).collect();
            let mut i = 0usize;
            while i < lines.len() {
                if lines[i].contains(pattern) {
                    let start = i.saturating_sub(context);
                    let end = (i + context + 1).min(lines.len());
                    out.push(format!("{rel}:{}", i + 1));
                    for (j, l) in lines[start..end].iter().enumerate() {
                        let lnum = start + j + 1;
                        let marker = if start + j == i { ">" } else { " " };
                        out.push(format!("{marker}{lnum:4}: {l}"));
                    }
                    out.push(String::new());
                    i = end;
                } else {
                    i += 1;
                }
            }
        }
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
    fn read_file_ok() {
        let d = tmp();
        std::fs::write(d.path().join("f.txt"), "hello\nworld\n").unwrap();
        let (out, err) = tool_read_file(&j(r#"{"path":"f.txt"}"#), d.path());
        assert!(!err, "{out}");
        assert!(out.contains("hello"));
    }

    #[test]
    fn read_file_missing() {
        let d = tmp();
        let (_, err) = tool_read_file(&j(r#"{"path":"nope.txt"}"#), d.path());
        assert!(err);
    }

    #[test]
    fn list_directory_shows_files() {
        let d = tmp();
        std::fs::write(d.path().join("a.rs"), "").unwrap();
        std::fs::write(d.path().join("b.rs"), "").unwrap();
        let path_str = d.path().to_string_lossy().replace('\\', "/");
        let (out, err) = tool_list_directory(
            &serde_json::json!({"path": path_str}),
            d.path(),
        );
        assert!(!err, "{out}");
        assert!(out.contains("a.rs") && out.contains("b.rs"));
    }

    #[test]
    fn find_files_glob_rs() {
        let d = tmp();
        std::fs::create_dir(d.path().join("src")).unwrap();
        std::fs::write(d.path().join("src/main.rs"), "").unwrap();
        std::fs::write(d.path().join("build.sh"), "").unwrap();
        let (out, err) = tool_find_files(&j(r#"{"pattern":"**/*.rs"}"#), d.path());
        assert!(!err, "{out}");
        assert!(out.contains("main.rs") && !out.contains("build.sh"));
    }

    #[test]
    fn grep_finds_pattern() {
        let d = tmp();
        std::fs::write(d.path().join("code.rs"), "fn main() {\n  println!(\"hello\");\n}\n").unwrap();
        let (out, err) = tool_grep(&j(r#"{"pattern":"println"}"#), d.path());
        assert!(!err, "{out}");
        assert!(out.contains("println"));
    }

    #[test]
    fn glob_double_star_matches_nested() {
        assert!(glob_match("**/*.rs", "src/main.rs"));
        assert!(glob_match("**/*.rs", "a/b/c.rs"));
        assert!(!glob_match("**/*.rs", "src/main.ts"));
    }
}
