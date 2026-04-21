//! Sidecar markdown: YAML frontmatter + prose sections (M21).

use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::ParseError;

/// YAML frontmatter for a per-file sidecar.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Frontmatter {
    /// Workspace-relative path (POSIX preferred in serialized form).
    pub source_path: PathBuf,
    #[serde(default)]
    pub source_hash: String,
    #[serde(default)]
    pub generated_by_model: String,
    #[serde(default)]
    pub generated_at: Option<DateTime<Utc>>,
    pub last_updated: DateTime<Utc>,
    #[serde(default)]
    pub dependencies: Vec<String>,
    #[serde(default)]
    pub references: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub summary: String,
}

/// Markdown body sections (machine round-trip + human-readable file).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct SidecarBody {
    #[serde(default)]
    pub reasoning: String,
    #[serde(default)]
    pub history: Vec<HistoryEntry>,
    #[serde(default)]
    pub notes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HistoryEntry {
    pub timestamp: DateTime<Utc>,
    pub summary: String,
    pub session_id: String,
}

/// Full sidecar document.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Sidecar {
    pub frontmatter: Frontmatter,
    pub body: SidecarBody,
}

#[must_use]
pub fn blank_sidecar(source_rel: &Path, source_text: &str, model: &str) -> Sidecar {
    let mut hasher = Sha256::new();
    hasher.update(source_text.as_bytes());
    let hash = format!("{:x}", hasher.finalize());
    let now = Utc::now();
    Sidecar {
        frontmatter: Frontmatter {
            source_path: source_rel.to_path_buf(),
            source_hash: hash,
            generated_by_model: model.to_string(),
            generated_at: Some(now),
            last_updated: now,
            dependencies: Vec::new(),
            references: Vec::new(),
            tags: Vec::new(),
            summary: String::new(),
        },
        body: SidecarBody::default(),
    }
}

/// Parse `---` / YAML / `---` / markdown body.
pub fn parse(raw: &str) -> Result<Sidecar, ParseError> {
    let trimmed = raw.trim_start_matches('\u{feff}').trim_start();
    let rest =
        trimmed.strip_prefix("---").ok_or(ParseError::MissingFrontmatter)?.trim_start_matches('\n');
    let (yaml_block, after_yaml) =
        rest.split_once("\n---").ok_or(ParseError::MissingFrontmatter)?;
    let mut fm: Frontmatter = serde_yaml::from_str(yaml_block.trim())?;
    let body_md = after_yaml.trim_start_matches('\n').trim_start();
    let body = parse_body_sections(body_md, &mut fm);
    Ok(Sidecar { frontmatter: fm, body })
}

#[derive(Copy, Clone)]
enum BodySection {
    None,
    Summary,
    Reasoning,
    History,
    Notes,
}

fn parse_body_sections(body_md: &str, fm: &mut Frontmatter) -> SidecarBody {
    let mut body = SidecarBody::default();
    let mut current = BodySection::None;
    let mut buf = String::new();

    fn flush_body(body: &mut SidecarBody, fm: &mut Frontmatter, sec: BodySection, text: &str) {
        let t = text.trim();
        if t.is_empty() {
            return;
        }
        match sec {
            BodySection::Summary => {
                if fm.summary.is_empty() {
                    fm.summary = t.to_string();
                }
            }
            BodySection::Reasoning => body.reasoning = t.to_string(),
            BodySection::History => {
                for line in t.lines() {
                    let line = line.trim();
                    let Some(rest) = line.strip_prefix('-').map(str::trim) else {
                        continue;
                    };
                    let parts: Vec<&str> = rest.splitn(3, '|').map(str::trim).collect();
                    if parts.len() == 3 {
                        if let Ok(ts_p) = parts[0].parse::<DateTime<Utc>>() {
                            body.history.push(HistoryEntry {
                                timestamp: ts_p,
                                session_id: parts[1].to_string(),
                                summary: parts[2].to_string(),
                            });
                        }
                    }
                }
            }
            BodySection::Notes => body.notes = t.to_string(),
            BodySection::None => {}
        }
    }

    for line in body_md.lines() {
        if line.starts_with("## Summary") {
            flush_body(&mut body, fm, current, &buf);
            buf.clear();
            current = BodySection::Summary;
            continue;
        }
        if line.starts_with("## Reasoning") {
            flush_body(&mut body, fm, current, &buf);
            buf.clear();
            current = BodySection::Reasoning;
            continue;
        }
        if line.starts_with("## History") {
            flush_body(&mut body, fm, current, &buf);
            buf.clear();
            current = BodySection::History;
            continue;
        }
        if line.starts_with("## Dependencies") || line.starts_with("## References") {
            flush_body(&mut body, fm, current, &buf);
            buf.clear();
            current = BodySection::None;
            continue;
        }
        if line.starts_with("## Notes") {
            flush_body(&mut body, fm, current, &buf);
            buf.clear();
            current = BodySection::Notes;
            continue;
        }
        if line.starts_with('#') && matches!(current, BodySection::None) {
            continue;
        }
        if !buf.is_empty() {
            buf.push('\n');
        }
        buf.push_str(line);
    }
    flush_body(&mut body, fm, current, &buf);
    body
}

/// Serialize to markdown suitable for disk and for model prompts.
pub fn write_to_markdown(sc: &Sidecar) -> Result<String, ParseError> {
    let yaml = serde_yaml::to_string(&sc.frontmatter).map_err(ParseError::Yaml)?;
    let title = posix_display(&sc.frontmatter.source_path);
    let mut out = String::new();
    out.push_str("---\n");
    out.push_str(&yaml);
    out.push_str("---\n\n");
    out.push_str(&format!("# `{title}`\n\n"));
    out.push_str("## Summary\n\n");
    out.push_str(&sc.frontmatter.summary);
    if !sc.frontmatter.summary.ends_with('\n') {
        out.push('\n');
    }
    out.push_str("\n## Reasoning\n\n");
    out.push_str(&sc.body.reasoning);
    if !sc.body.reasoning.ends_with('\n') {
        out.push('\n');
    }
    out.push_str("\n## History\n\n");
    for h in &sc.body.history {
        out.push_str(&format!(
            "- {} | {} | {}\n",
            h.timestamp.to_rfc3339(),
            h.session_id,
            h.summary
        ));
    }
    if !sc.body.notes.is_empty() {
        out.push_str("\n## Notes\n\n");
        out.push_str(&sc.body.notes);
        if !sc.body.notes.ends_with('\n') {
            out.push('\n');
        }
    }
    Ok(out)
}

fn posix_display(p: &Path) -> String {
    p.to_string_lossy().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_blank() {
        let sc = blank_sidecar(Path::new("src/x.rs"), "fn main() {}", "test");
        let md = write_to_markdown(&sc).unwrap();
        let sc2 = parse(&md).unwrap();
        assert_eq!(sc.frontmatter.source_path, sc2.frontmatter.source_path);
        assert_eq!(sc.frontmatter.source_hash, sc2.frontmatter.source_hash);
    }
}
