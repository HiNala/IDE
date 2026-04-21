//! Read-only git integration (M18): discover repo, branch name, file status vs `HEAD`, `HEAD` blob text.

#![forbid(unsafe_code)]

mod repo;

pub use repo::{FileStatus, GitError, GitRepo};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
