//! Monospace width fitting helpers for chrome labels (tab titles, chips, …).

/// Ellipsis to fit `max_width_px` using `char_w * scale` per character (same convention as tab strip).
#[must_use]
pub fn ellipsize_mono(s: &str, max_width_px: f32, scale: f32, char_w: f32) -> String {
    if max_width_px <= 0.0 || s.is_empty() {
        return String::new();
    }
    let cw = char_w * scale;
    let len = s.chars().count();
    if len == 0 {
        return String::new();
    }
    if (len as f32) * cw <= max_width_px {
        return s.to_string();
    }
    // One codepoint for "…" at the end.
    let ellipsis_w = cw;
    let budget_w = (max_width_px - ellipsis_w).max(0.0);
    let max_chars = (budget_w / cw).floor() as usize;
    if max_chars == 0 {
        return "…".to_string();
    }
    let prefix: String = s.chars().take(max_chars).collect();
    if prefix.chars().count() < len {
        format!("{prefix}…")
    } else {
        prefix
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ellipsize_trims_long_string() {
        let s = ellipsize_mono("hello_world.rs", 50.0, 1.0, 7.2);
        assert!(s.ends_with('…'));
        assert!(s.len() < "hello_world.rs".len());
    }
}
