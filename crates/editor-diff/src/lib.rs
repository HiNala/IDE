//! Line and intra-line diff (similar-based) for the IDE project.

#![forbid(unsafe_code)]

mod compute;
mod display;
mod intra_line;
mod session;
mod types;

pub use compute::compute_line_diff;
pub use display::{DiffGutter, DiffPaint, InlineDiffDocument, InlineDiffLine};
pub use session::{apply_hunk_to_buffer, DiffReviewState};
pub use types::*;
