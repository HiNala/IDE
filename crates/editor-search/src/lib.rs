//! Find / replace for a single buffer and streaming workspace search (M16).
//!
//! - In-file: [`in_file::search_buffer`], [`in_file::replace_one`], [`in_file::replace_all`].
//! - Project: [`project::start_project_search`] with [`project::SearchEvent`] streaming.

#![forbid(unsafe_code)]

pub mod error;
pub mod in_file;
pub mod project;

pub use error::SearchError;
pub use in_file::{
    build_pattern, replace_all, replace_one, search_buffer, InFileMatch, InFileSearch,
    InFileSearchResult, IN_FILE_MATCH_CAP,
};
pub use project::{start_project_search, ProjectMatch, ProjectSearch, SearchEvent, SearchJob};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[must_use]
pub fn banner() -> String {
    format!("editor-search v{VERSION}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn banner_ok() {
        assert!(banner().contains("editor-search"));
    }
}
