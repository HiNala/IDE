//! Repository discovery and read-only queries against `HEAD` (worktree vs committed tree).

use std::path::{Path, PathBuf};

use gix::object::tree::EntryKind;
use thiserror::Error;

/// Worktree state for a single path compared to the tree at `HEAD` (last commit).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileStatus {
    /// Present in `HEAD` and the worktree with identical bytes.
    Unmodified,
    /// Present in both but content differs.
    Modified,
    /// Exists in the worktree but not in the `HEAD` tree.
    Added,
    /// Listed in the `HEAD` tree but missing from the worktree.
    Removed,
}

#[derive(Debug, Error)]
pub enum GitError {
    /// Boxed so `Result<_, GitError>` stays small (`clippy::result_large_err`).
    #[error("git discovery: {0}")]
    Discover(Box<gix::discover::Error>),
    #[error("git: {0}")]
    Operation(String),
    #[error("repository has no working tree (bare repo)")]
    BareRepository,
    #[error("path `{0}` escapes the repository worktree")]
    PathEscapesWorktree(PathBuf),
    #[error("HEAD entry for `{0}` is not a blob or symlink")]
    UnsupportedHeadEntry(PathBuf),
}

impl From<gix::discover::Error> for GitError {
    fn from(err: gix::discover::Error) -> Self {
        Self::Discover(Box::new(err))
    }
}

/// Read-only handle to an on-disk Git repository (non-bare).
#[derive(Debug)]
pub struct GitRepo {
    repo: gix::Repository,
    workdir: PathBuf,
}

impl GitRepo {
    /// Open a repository by walking up from `start`, or return `Ok(None)` if none is found.
    pub fn discover(start: impl AsRef<Path>) -> Result<Option<Self>, GitError> {
        match gix::discover(start.as_ref()) {
            Ok(repo) => Ok(Some(Self::new(repo)?)),
            Err(err) if is_no_git_repository(&err) => Ok(None),
            Err(err) => Err(err.into()),
        }
    }

    fn new(repo: gix::Repository) -> Result<Self, GitError> {
        let workdir = repo.work_dir().map(Path::to_path_buf).ok_or(GitError::BareRepository)?;
        Ok(Self { repo, workdir })
    }

    /// Root of the working tree (normalized absolute path from `gix`).
    #[must_use]
    pub fn workdir(&self) -> &Path {
        &self.workdir
    }

    /// Short branch name when `HEAD` is attached; `None` when detached or unborn.
    #[must_use]
    pub fn branch_name(&self) -> Option<String> {
        let name = self.repo.head_name().ok()??;
        Some(name.shorten().to_string())
    }

    /// UTF-8 text from the `HEAD` blob at `relative_path`, or `None` if the path is not in `HEAD`.
    ///
    /// Decoding is lossy; non-UTF-8 bytes are replaced.
    pub fn head_blob_text_lossy(
        &self,
        relative_path: impl AsRef<Path>,
    ) -> Result<Option<String>, GitError> {
        let rel = self.normalize_relative(relative_path.as_ref())?;
        let mut buf = Vec::new();
        let Some(bytes) = self.head_entry_bytes(&rel, &mut buf)? else {
            return Ok(None);
        };
        Ok(Some(String::from_utf8_lossy(&bytes).into_owned()))
    }

    /// Compare the worktree file at `relative_path` to the `HEAD` tree.
    pub fn file_status_vs_head(
        &self,
        relative_path: impl AsRef<Path>,
    ) -> Result<FileStatus, GitError> {
        let rel = self.normalize_relative(relative_path.as_ref())?;
        let abs = self.workdir.join(&rel);
        let worktree_bytes = std::fs::read(&abs);
        let worktree_ok = worktree_bytes.is_ok();

        let mut buf = Vec::new();
        let head_bytes = self.head_entry_bytes(&rel, &mut buf)?;

        match (head_bytes, worktree_ok) {
            (None, false) => Err(GitError::Operation(format!(
                "path has no worktree file and is not in HEAD: {}",
                rel.display()
            ))),
            (None, true) => Ok(FileStatus::Added),
            (Some(_), false) => Ok(FileStatus::Removed),
            (Some(h), true) => {
                let w = worktree_bytes.expect("checked");
                Ok(if w == h { FileStatus::Unmodified } else { FileStatus::Modified })
            }
        }
    }

    /// Line-oriented diff of `worktree_text` against the `HEAD` revision of `relative_path`.
    ///
    /// If the path is missing from `HEAD`, the "before" side is treated as empty.
    pub fn line_diff_vs_head(
        &self,
        relative_path: impl AsRef<Path>,
        worktree_text: &str,
    ) -> Result<Vec<editor_diff::Hunk>, GitError> {
        let before = self.head_blob_text_lossy(relative_path)?.unwrap_or_default();
        Ok(editor_diff::compute_line_diff(&before, worktree_text))
    }

    fn normalize_relative(&self, path: &Path) -> Result<PathBuf, GitError> {
        let path = if path.is_absolute() {
            path.strip_prefix(&self.workdir)
                .map_err(|_| GitError::PathEscapesWorktree(path.to_path_buf()))?
                .to_path_buf()
        } else {
            path.to_path_buf()
        };
        Ok(path)
    }

    fn head_entry_bytes(
        &self,
        relative: &Path,
        buf: &mut Vec<u8>,
    ) -> Result<Option<Vec<u8>>, GitError> {
        let commit = match self.repo.head_commit() {
            Ok(c) => c,
            Err(_) => return Ok(None),
        };
        let tree = commit.tree().map_err(|e| GitError::Operation(e.to_string()))?;
        let entry = tree
            .lookup_entry_by_path(relative, buf)
            .map_err(|e| GitError::Operation(e.to_string()))?;
        let Some(entry) = entry else {
            return Ok(None);
        };
        match entry.mode().kind() {
            EntryKind::Blob | EntryKind::Link => {
                let blob = entry
                    .object()
                    .map_err(|e| GitError::Operation(e.to_string()))?
                    .try_into_blob()
                    .map_err(|e| GitError::Operation(e.to_string()))?;
                Ok(Some(blob.data.to_vec()))
            }
            _ => Err(GitError::UnsupportedHeadEntry(relative.to_path_buf())),
        }
    }
}

fn is_no_git_repository(err: &gix::discover::Error) -> bool {
    matches!(
        err,
        gix::discover::Error::Discover(e) if matches!(
            e,
            gix::discover::upwards::Error::NoGitRepository { .. }
                | gix::discover::upwards::Error::NoGitRepositoryWithinCeiling { .. }
                | gix::discover::upwards::Error::NoGitRepositoryWithinFs { .. }
        )
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discover_finds_self_repo() {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let repo = GitRepo::discover(&manifest_dir)
            .expect("discover")
            .expect("this crate lives in a git checkout");
        assert!(repo.workdir().join("Cargo.toml").is_file());
    }
}
