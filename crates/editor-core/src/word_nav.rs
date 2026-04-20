//! Word-level navigation using UAX #29 word boundaries (`unicode-segmentation`).
//!
//! Moving left/right jumps between word segments from [`UnicodeSegmentation::split_word_bound_indices`].

use std::ops::Range;

use unicode_segmentation::UnicodeSegmentation;

/// Byte index for **Ctrl+Left** / **Alt+Left** (word left).
///
/// Moves to the start of the current “word” (Unicode word segment that is not
/// whitespace-only), or to the start of the previous such word when the cursor
/// is already at a word boundary or in whitespace — matching typical editor UX
/// (skipping runs of spaces between words).
#[must_use]
pub fn word_left(s: &str, cursor_byte: usize) -> usize {
    let cursor_byte = cursor_byte.min(s.len());
    let segments: Vec<(usize, &str)> = s.split_word_bound_indices().collect();
    if segments.is_empty() {
        return 0;
    }

    /// Non-whitespace-only segments: `(start_byte, end_byte)`.
    fn word_spans(segments: &[(usize, &str)]) -> Vec<(usize, usize)> {
        segments
            .iter()
            .filter(|(_, t)| !t.is_empty() && !t.chars().all(|c| c.is_whitespace()))
            .map(|(start, t)| (*start, *start + t.len()))
            .collect()
    }

    let words = word_spans(&segments);
    if words.is_empty() {
        return 0;
    }

    // Cursor strictly inside a word → that word's start.
    for &(start, end) in &words {
        if cursor_byte > start && cursor_byte <= end {
            return start;
        }
    }

    // Cursor at a word's first byte: previous word's start (or 0).
    for (i, &(start, _)) in words.iter().enumerate() {
        if cursor_byte == start {
            return if i == 0 { 0 } else { words[i - 1].0 };
        }
    }

    // Cursor in whitespace-only segments, or before first word: previous word start.
    for (i, &(start, end)) in words.iter().enumerate() {
        if cursor_byte < start {
            return if i == 0 { 0 } else { words[i - 1].0 };
        }
        if cursor_byte > end && i + 1 < words.len() && cursor_byte < words[i + 1].0 {
            return start;
        }
    }

    // After last word (e.g. trailing newline) → last word start.
    let last = words[words.len() - 1];
    if cursor_byte > last.1 {
        return last.0;
    }

    0
}

/// Byte index for **Ctrl+Right** / **Alt+Right** (word right).
#[must_use]
pub fn word_right(s: &str, cursor_byte: usize) -> usize {
    let cursor_byte = cursor_byte.min(s.len());
    let segments: Vec<(usize, &str)> = s.split_word_bound_indices().collect();
    if segments.is_empty() {
        return s.len();
    }
    for (start, word) in &segments {
        let end = start + word.len();
        if cursor_byte < end {
            return end;
        }
    }
    s.len()
}

/// Byte range for **Ctrl+Backspace** / **Alt+Backspace**.
#[must_use]
pub fn delete_word_backward_range(s: &str, cursor_byte: usize) -> Option<Range<usize>> {
    let end = cursor_byte.min(s.len());
    if end == 0 {
        return None;
    }
    let start = word_left(s, end);
    (start < end).then_some(start..end)
}

/// Byte range for **Ctrl+Delete** / **Alt+Delete**.
#[must_use]
pub fn delete_word_forward_range(s: &str, cursor_byte: usize) -> Option<Range<usize>> {
    let start = cursor_byte.min(s.len());
    if start >= s.len() {
        return None;
    }
    let end = word_right(s, start);
    (start < end).then_some(start..end)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn byte_at(s: &str, char_idx: usize) -> usize {
        s.char_indices().nth(char_idx).map(|(b, _)| b).unwrap_or(s.len())
    }

    #[test]
    fn word_left_ascii() {
        let s = "hello world";
        assert_eq!(word_left(s, s.len()), byte_at(s, 6));
        assert_eq!(word_left(s, byte_at(s, 6)), 0);
        assert_eq!(word_left(s, 0), 0);
    }

    #[test]
    fn word_right_ascii() {
        let s = "hello world";
        assert_eq!(word_right(s, 0), 5);
        assert_eq!(word_right(s, 2), 5);
        assert_eq!(word_right(s, 5), 6);
        assert_eq!(word_right(s, 6), s.len());
        assert_eq!(word_right(s, s.len()), s.len());
    }

    #[test]
    fn cjk_segments() {
        let s = "你好世界";
        assert!(word_left(s, s.len()) < s.len());
        assert!(word_right(s, 0) > 0);
    }

    #[test]
    fn delete_word_backward_basic() {
        let s = "hello world";
        let c = s.len();
        let r = delete_word_backward_range(s, c).expect("range");
        assert_eq!(&s[r.start..r.end], "world");
    }

    #[test]
    fn delete_word_forward_basic() {
        let s = "hello world";
        let r = delete_word_forward_range(s, 0).expect("range");
        assert_eq!(&s[r.start..r.end], "hello");
    }
}
