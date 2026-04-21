//! Filesystem layout under `.ide/meta/`.

use std::path::{Path, PathBuf};

use crate::error::{MetadataError, Result};
use crate::schema::{parse, write_to_markdown, Sidecar};

/// Manages `.ide/meta/<path-to-source>.md` sidecar files.
#[derive(Debug, Clone)]
pub struct MetadataStore {
    workspace_root: PathBuf,
    meta_root: PathBuf,
}

impl MetadataStore {
    #[must_use]
    pub fn new(workspace_root: PathBuf) -> Self {
        let meta_root = workspace_root.join(".ide").join("meta");
        Self { workspace_root, meta_root }
    }

    #[must_use]
    pub fn meta_root(&self) -> &Path {
        &self.meta_root
    }

    #[must_use]
    pub fn workspace_root(&self) -> &Path {
        &self.workspace_root
    }

    /// Maps `src/foo.rs` → `.ide/meta/src/foo.rs.md`
    #[must_use]
    pub fn sidecar_path(&self, source: &Path) -> PathBuf {
        let rel = source.strip_prefix(&self.workspace_root).unwrap_or(source);
        let parent = rel.parent();
        let fname = rel.file_name().map(|f| f.to_string_lossy()).unwrap_or_else(|| "".into());
        let file = format!("{fname}.md");
        match parent {
            Some(p) if p.as_os_str().is_empty() => self.meta_root.join(file),
            Some(p) => self.meta_root.join(p).join(file),
            None => self.meta_root.join(file),
        }
    }

    pub fn load(&self, source: &Path) -> Result<Option<Sidecar>> {
        let p = self.sidecar_path(source);
        if !p.is_file() {
            return Ok(None);
        }
        let raw = std::fs::read_to_string(&p)?;
        let sc = parse(&raw).map_err(MetadataError::Parse)?;
        Ok(Some(sc))
    }

    pub fn save(&self, sidecar: &Sidecar) -> Result<()> {
        let src = self.workspace_root.join(&sidecar.frontmatter.source_path);
        let path = self.sidecar_path(&src);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let md = write_to_markdown(sidecar).map_err(MetadataError::Parse)?;
        std::fs::write(&path, md)?;
        Ok(())
    }

    pub fn exists(&self, source: &Path) -> bool {
        self.sidecar_path(source).is_file()
    }

    pub fn delete(&self, source: &Path) -> Result<()> {
        let p = self.sidecar_path(source);
        if p.is_file() {
            std::fs::remove_file(&p)?;
        }
        Self::prune_empty_parents(&p, &self.meta_root);
        Ok(())
    }

    fn prune_empty_parents(mut p: &Path, stop_at: &Path) {
        while let Some(par) = p.parent() {
            if par == stop_at || !par.starts_with(stop_at) {
                break;
            }
            if std::fs::read_dir(par).map(|mut d| d.next().is_none()).unwrap_or(false) {
                let _ = std::fs::remove_dir(par);
            } else {
                break;
            }
            p = par;
        }
    }

    /// All `*.md` paths under `.ide/meta/`.
    pub fn list_all(&self) -> Result<Vec<PathBuf>> {
        let mut out = Vec::new();
        if !self.meta_root.is_dir() {
            return Ok(out);
        }
        collect_md(&self.meta_root, &mut out).map_err(MetadataError::Io)?;
        Ok(out)
    }
}

fn collect_md(dir: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for e in std::fs::read_dir(dir)? {
        let e = e?;
        let p = e.path();
        let ft = e.file_type()?;
        if ft.is_dir() {
            collect_md(&p, out)?;
        } else if ft.is_file() && p.extension().is_some_and(|x| x == "md") {
            out.push(p);
        }
    }
    Ok(())
}
