//! Line ending detection and normalization (LF / CRLF / CR / mixed).

/// Detected or target line-ending convention.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineEnding {
    /// Unix / modern macOS (`"\n"`).
    Lf,
    /// Windows (`"\r\n"`).
    Crlf,
    /// Classic Mac (rare today) (`"\r"`).
    Cr,
    /// More than one convention appears in the sample.
    Mixed,
}

impl LineEnding {
    /// Scan up to the first ~64 KiB and infer the dominant convention.
    ///
    /// If two different conventions appear, returns [`LineEnding::Mixed`].
    #[must_use]
    pub fn detect(text: &str) -> Self {
        let sample = text.chars().take(64 * 1024).collect::<String>();
        let mut saw_lf = false;
        let mut saw_crlf = false;
        let mut saw_cr_alone = false;
        let mut chars = sample.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '\r' {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                    saw_crlf = true;
                } else {
                    saw_cr_alone = true;
                }
            } else if c == '\n' {
                saw_lf = true;
            }
        }
        let kinds = usize::from(saw_crlf) + usize::from(saw_lf) + usize::from(saw_cr_alone);
        if kinds > 1 {
            return Self::Mixed;
        }
        if saw_crlf {
            return Self::Crlf;
        }
        if saw_cr_alone {
            return Self::Cr;
        }
        Self::Lf
    }

    /// Preferred on-disk representation for this convention (single line break).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Lf | Self::Mixed => "\n",
            Self::Crlf => "\r\n",
            Self::Cr => "\r",
        }
    }

    /// Convert all line endings in `input` to the target convention's logical
    /// newlines. For [`LineEnding::Lf`], internal storage is plain `'\n'`.
    #[must_use]
    pub fn normalize_to(target: Self, input: &str) -> String {
        match target {
            Self::Lf | Self::Mixed => normalize_to_lf(input),
            Self::Crlf => normalize_to_string(input, "\r\n"),
            Self::Cr => normalize_to_string(input, "\r"),
        }
    }
}

/// Internal LF form used by [`crate::buffer::TextBuffer`].
#[must_use]
pub(crate) fn normalize_to_lf(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\r' {
            if chars.peek() == Some(&'\n') {
                chars.next();
            }
            out.push('\n');
        } else {
            out.push(c);
        }
    }
    out
}

fn normalize_to_string(input: &str, newline: &'static str) -> String {
    let lf = normalize_to_lf(input);
    if newline == "\n" {
        return lf;
    }
    lf.replace('\n', newline)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_empty() {
        assert_eq!(LineEnding::detect(""), LineEnding::Lf);
    }

    #[test]
    fn detect_lf() {
        assert_eq!(LineEnding::detect("a\nb\n"), LineEnding::Lf);
    }

    #[test]
    fn detect_crlf() {
        assert_eq!(LineEnding::detect("a\r\nb\r\n"), LineEnding::Crlf);
    }

    #[test]
    fn detect_cr() {
        assert_eq!(LineEnding::detect("a\rb\r"), LineEnding::Cr);
    }

    #[test]
    fn detect_mixed() {
        assert_eq!(LineEnding::detect("a\nb\r\nc"), LineEnding::Mixed);
    }

    #[test]
    fn normalize_lf_idempotent() {
        let s = "x\ny\n";
        assert_eq!(LineEnding::normalize_to(LineEnding::Lf, s), s);
    }
}
