//! Canonical workspace-relative path resolution (security boundary).

use std::path::{Component, Path, PathBuf};

use crate::error::{Result, ToolError};

/// Resolve `relative` under `workspace_root` and ensure the result stays inside the workspace.
///
/// `workspace_root` should be canonical (see [`editor_workspace::Workspace::open`]). Works for
/// paths whose final components do not exist yet by canonicalizing the longest existing prefix and
/// appending the remainder, then verifying the resolved prefix stays under the root (catches
/// symlink escapes).
pub fn canonical_under_workspace(workspace_root: &Path, relative: &str) -> Result<PathBuf> {
    let trimmed = relative.trim();
    if trimmed.is_empty() {
        return Err(ToolError::msg("empty path"));
    }
    let rel = Path::new(trimmed);
    if rel.is_absolute() {
        return Err(ToolError::PathEscape(trimmed.into()));
    }

    let root = workspace_root
        .canonicalize()
        .map_err(|e| ToolError::InvalidPath(format!("workspace root {workspace_root:?}: {e}")))?;

    let mut built = root.clone();
    for c in rel.components() {
        match c {
            Component::Normal(part) => built.push(part),
            Component::CurDir => {}
            Component::ParentDir => {
                if !built.pop() {
                    return Err(ToolError::PathEscape(trimmed.into()));
                }
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err(ToolError::PathEscape(trimmed.into()));
            }
        }
    }

    if !built.starts_with(&root) {
        return Err(ToolError::PathEscape(trimmed.into()));
    }

    let mut prefix = built.clone();
    let mut rest = PathBuf::new();
    loop {
        if prefix.exists() {
            let canon_prefix = prefix
                .canonicalize()
                .map_err(|e| ToolError::InvalidPath(format!("{prefix:?}: {e}")))?;
            if !canon_prefix.starts_with(&root) {
                return Err(ToolError::PathEscape(trimmed.into()));
            }
            return Ok(if rest.as_os_str().is_empty() {
                canon_prefix
            } else {
                canon_prefix.join(&rest)
            });
        }
        if let Some(name) = prefix.file_name() {
            rest = Path::new(name).join(&rest);
        }
        if !prefix.pop() {
            return Err(ToolError::InvalidPath(format!(
                "could not resolve path under workspace: {trimmed}"
            )));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn rejects_parent_escape() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().canonicalize().unwrap();
        let r = canonical_under_workspace(&root, "../../etc/passwd");
        assert!(matches!(r, Err(ToolError::PathEscape(_))));
    }

    #[test]
    fn accepts_normal_relative() {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir_all(tmp.path().join("src")).unwrap();
        fs::write(tmp.path().join("src/main.rs"), "x").unwrap();
        let root = tmp.path().canonicalize().unwrap();
        let p = canonical_under_workspace(&root, "src/main.rs").unwrap();
        assert!(p.starts_with(&root));
    }

    #[test]
    fn accepts_new_file_under_existing_dir() {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir_all(tmp.path().join("a")).unwrap();
        let root = tmp.path().canonicalize().unwrap();
        let p = canonical_under_workspace(&root, "a/new.txt").unwrap();
        assert_eq!(p.file_name().unwrap(), "new.txt");
        assert!(p.starts_with(&root));
    }

    /// Symlink under the workspace that points *outside* must fail after canonicalization.
    #[cfg(unix)]
    #[test]
    fn rejects_symlink_to_path_outside_workspace() {
        use std::os::unix::fs::symlink;

        let tmp = tempfile::tempdir().unwrap();
        let ws = tmp.path().join("ws");
        fs::create_dir_all(&ws).unwrap();
        let secret = tmp.path().join("outside_secret.txt");
        fs::write(&secret, "x").unwrap();
        let link = ws.join("leak");
        symlink(&secret, &link).unwrap();

        let root = ws.canonicalize().unwrap();
        let r = canonical_under_workspace(&root, "leak");
        assert!(matches!(r, Err(ToolError::PathEscape(_))), "expected PathEscape, got {r:?}");
    }
}
