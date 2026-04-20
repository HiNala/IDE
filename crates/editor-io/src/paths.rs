//! Cross-platform path helpers.

use std::path::Path;

/// `true` if the final path component matches a Windows reserved device name.
#[must_use]
pub fn is_windows_reserved_path(path: &Path) -> bool {
    if !cfg!(target_os = "windows") {
        return false;
    }
    let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
        return false;
    };
    // Strip extension: `con.txt` is reserved.
    let stem = Path::new(name).file_stem().and_then(|s| s.to_str()).unwrap_or(name);
    let upper = stem.to_ascii_uppercase();
    matches!(
        upper.as_str(),
        "CON"
            | "PRN"
            | "AUX"
            | "NUL"
            | "COM1"
            | "COM2"
            | "COM3"
            | "COM4"
            | "COM5"
            | "COM6"
            | "COM7"
            | "COM8"
            | "COM9"
            | "LPT1"
            | "LPT2"
            | "LPT3"
            | "LPT4"
            | "LPT5"
            | "LPT6"
            | "LPT7"
            | "LPT8"
            | "LPT9"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn reserved_con() {
        if cfg!(target_os = "windows") {
            assert!(is_windows_reserved_path(&PathBuf::from("CON")));
            assert!(is_windows_reserved_path(&PathBuf::from(r"C:\foo\con.txt")));
        } else {
            assert!(!is_windows_reserved_path(&PathBuf::from("CON")));
        }
    }

    #[test]
    fn not_reserved() {
        assert!(!is_windows_reserved_path(&PathBuf::from("console.txt")));
    }
}
