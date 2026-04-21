//! Tree-sitter–based definition chunks (Rust first).

use std::path::Path;

use sha2::{Digest, Sha256};
use tree_sitter::{Node, Parser};

use crate::schema::{Chunk, ChunkKind, ChunkMetadata};

const MAX_DEF_SNIPPET: usize = 1200;
const MAX_PER_FILE: usize = 200;

pub fn source_file_hash(content: &str) -> String {
    let mut h = Sha256::new();
    h.update(content.as_bytes());
    format!("{:x}", h.finalize())
}

/// Extract top-level Rust definitions as candidate chunks.
pub fn extract_rust_definitions(
    source: &str,
    workspace_relative: &Path,
    embedder_id: impl Into<String>,
) -> Vec<Chunk> {
    let mut parser = Parser::new();
    let lang: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
    if parser.set_language(&lang).is_err() {
        return Vec::new();
    }
    let Some(tree) = parser.parse(source, None) else {
        return Vec::new();
    };
    let embedder_id: String = embedder_id.into();
    let root = tree.root_node();
    let mut out = Vec::new();
    for i in 0..root.child_count() {
        if out.len() >= MAX_PER_FILE {
            break;
        }
        let node = match root.child(i) {
            Some(n) => n,
            None => continue,
        };
        let (kind, name) = match node.kind() {
            "function_item" => {
                ("fn".to_string(), rust_decl_name(source, &node).unwrap_or_else(|| "anon".into()))
            }
            "struct_item" => (
                "struct".to_string(),
                rust_decl_name(source, &node).unwrap_or_else(|| "anon".into()),
            ),
            "enum_item" => {
                ("enum".to_string(), rust_decl_name(source, &node).unwrap_or_else(|| "anon".into()))
            }
            "trait_item" => (
                "trait".to_string(),
                rust_decl_name(source, &node).unwrap_or_else(|| "anon".into()),
            ),
            "impl_item" => ("impl".to_string(), format!("{}", node.start_byte())),
            "mod_item" => {
                ("mod".to_string(), rust_decl_name(source, &node).unwrap_or_else(|| "anon".into()))
            }
            _ => continue,
        };
        let start = node.start_byte();
        let end = (start + MAX_DEF_SNIPPET).min(source.len());
        let snippet = source.get(start..end).unwrap_or("").trim();
        if snippet.is_empty() {
            continue;
        }
        let id = chunk_id_for_code(workspace_relative, &kind, &name, start as u32);
        let now = chrono::Utc::now();
        let hash = source_file_hash(source);
        out.push(Chunk {
            id,
            source_path: workspace_relative.to_path_buf(),
            chunk_kind: ChunkKind::CodeDefinition { kind, name: name.clone() },
            text: format!("{name}\n{snippet}"),
            source_hash: hash,
            metadata: ChunkMetadata {
                tags: Vec::new(),
                line_start: Some(node.start_position().row as u32 + 1),
                line_end: Some(node.end_position().row as u32 + 1),
                embedded_at: now,
                embedder_id: embedder_id.clone(),
            },
        });
    }
    out
}

fn rust_decl_name(source: &str, node: &Node<'_>) -> Option<String> {
    for i in 0..node.child_count() {
        let c = node.child(i)?;
        if ["type_identifier", "identifier", "field_identifier"].contains(&c.kind()) {
            return Some(c.utf8_text(source.as_bytes()).ok()?.to_string());
        }
    }
    None
}

fn chunk_id_for_code(rel: &Path, kind: &str, name: &str, start_byte: u32) -> String {
    let rel_s = rel.to_string_lossy();
    let mut h = Sha256::new();
    h.update(rel_s.as_bytes());
    h.update(b"|");
    h.update(kind.as_bytes());
    h.update(b"|");
    h.update(name.as_bytes());
    h.update(b"|");
    h.update(start_byte.to_le_bytes());
    format!("{:x}", h.finalize())
}

/// Returns true if the path should be scanned for code chunks.
#[must_use]
pub fn is_indexable_code_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|e| e.to_str()),
        Some("rs" | "py" | "go" | "c" | "h" | "cpp" | "hpp" | "js" | "ts" | "tsx" | "jsx")
    )
}

/// Skip generated / lockfiles by simple heuristics.
#[must_use]
pub fn should_skip_path(path: &Path) -> bool {
    let s = path.to_string_lossy();
    s.contains("target/")
        || s.contains("node_modules/")
        || s.ends_with("Cargo.lock")
        || s.ends_with(".min.js")
}
