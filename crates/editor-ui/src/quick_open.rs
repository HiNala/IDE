//! Fuzzy file ranking for **Ctrl+P** quick-open (M14).
//!
//! Uses [`nucleo::Matcher`] (same family as Helix). Matchers are large (~135 KiB scratch);
//! this module keeps one per [`QuickOpenRanker`] instance for reuse while the palette is open.

use nucleo::{Config, Matcher, Utf32String};

/// Ranks workspace file paths by fuzzy score for a search query.
///
/// - Empty `query`: returns the first `limit` indices in order (no scoring).
/// - Indices are into the original `paths` slice, sorted by **descending** match score.
#[derive(Debug)]
pub struct QuickOpenRanker {
    matcher: Matcher,
    hay_scratch: Utf32String,
    needle_scratch: Utf32String,
}

impl Default for QuickOpenRanker {
    fn default() -> Self {
        Self::new()
    }
}

impl QuickOpenRanker {
    /// Creates a ranker with default nucleo [`Config`].
    #[must_use]
    pub fn new() -> Self {
        Self {
            matcher: Matcher::new(Config::DEFAULT),
            hay_scratch: Utf32String::default(),
            needle_scratch: Utf32String::default(),
        }
    }

    /// Returns up to `limit` path indices best matching `query`.
    pub fn rank_paths(&mut self, query: &str, paths: &[String], limit: usize) -> Vec<usize> {
        if paths.is_empty() || limit == 0 {
            return Vec::new();
        }
        if query.is_empty() {
            return (0..paths.len().min(limit)).collect();
        }

        self.needle_scratch = Utf32String::from(query);
        let needle = self.needle_scratch.slice(..);

        let mut scored: Vec<(usize, u16)> = Vec::new();
        for (i, p) in paths.iter().enumerate() {
            self.hay_scratch = Utf32String::from(p.as_str());
            let hay = self.hay_scratch.slice(..);
            if let Some(score) = self.matcher.fuzzy_match(hay, needle) {
                scored.push((i, score));
            }
        }

        scored.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        scored.into_iter().take(limit).map(|(i, _)| i).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_query_returns_prefix() {
        let paths: Vec<String> = (0..5).map(|i| format!("p{i}.rs")).collect();
        let mut r = QuickOpenRanker::new();
        assert_eq!(r.rank_paths("", &paths, 3), vec![0, 1, 2]);
    }

    #[test]
    fn fuzzy_prefers_subpath_match() {
        let paths = vec![
            "foo.txt".to_string(),
            "crates/editor-core/src/rope.rs".to_string(),
            "other.rs".to_string(),
        ];
        let mut r = QuickOpenRanker::new();
        let got = r.rank_paths("rope", &paths, 2);
        assert_eq!(got[0], 1);
    }

    #[test]
    fn empty_paths_or_limit_returns_nothing() {
        let paths: Vec<String> = vec!["a.rs".into()];
        let mut r = QuickOpenRanker::new();
        assert!(r.rank_paths("a", &[], 5).is_empty());
        assert!(r.rank_paths("a", &paths, 0).is_empty());
    }

    #[test]
    fn no_fuzzy_match_returns_empty() {
        let paths = vec!["aaa.txt".into(), "bbb.txt".into()];
        let mut r = QuickOpenRanker::new();
        assert!(r.rank_paths("qqqzzz", &paths, 5).is_empty());
    }
}
