//! Dynamic `system-info` skill body: OS, shell, tool versions, coarse resources.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use editor_terminal::detect_shell;

const CACHE_TTL: Duration = Duration::from_secs(5 * 60);

struct Cache {
    workspace: Option<String>,
    generated_at: Instant,
    body: String,
}

static CACHE: Mutex<Option<Cache>> = Mutex::new(None);

/// Invalidate cached system-info (call on workspace open/close).
pub fn invalidate_system_info_cache() {
    let mut g = CACHE.lock().expect("system-info cache mutex poisoned");
    *g = None;
}

/// Generate markdown body for the `system-info` skill.
pub fn generate_system_info_body(workspace_root: Option<&Path>) -> String {
    let ws_key = workspace_root.map(|p| p.display().to_string());
    {
        let g = CACHE.lock().expect("system-info cache mutex poisoned");
        if let Some(c) = g.as_ref() {
            if c.workspace == ws_key && c.generated_at.elapsed() < CACHE_TTL {
                return c.body.clone();
            }
        }
    }

    let body = build_body(workspace_root);
    {
        let mut g = CACHE.lock().expect("system-info cache mutex poisoned");
        *g = Some(Cache { workspace: ws_key, generated_at: Instant::now(), body: body.clone() });
    }
    body
}

fn git_toplevel(root: &Path) -> Option<PathBuf> {
    let out = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .current_dir(root)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if s.is_empty() {
        None
    } else {
        Some(PathBuf::from(s))
    }
}

fn build_body(workspace_root: Option<&Path>) -> String {
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;
    let family = std::env::consts::FAMILY;
    let shell_line = match detect_shell(None) {
        Ok(cfg) => format!("{}", cfg.program.display()),
        Err(e) => format!("(unknown: {e})"),
    };

    let mut out = String::new();
    out.push_str("# System environment (dynamic)\n\n");
    out.push_str("This skill is generated at load time; call `load_skill(\"system-info\")` when the task depends on the machine.\n\n");
    out.push_str("## Platform\n\n");
    out.push_str(&format!("- **OS:** `{os}` (family `{family}`)\n"));
    out.push_str(&format!("- **Arch:** `{arch}`\n"));
    out.push_str(&format!("- **Shell:** {shell_line}\n"));

    if let Some(root) = workspace_root {
        out.push_str("\n## Workspace\n\n");
        out.push_str(&format!("- **Root:** `{}`\n", root.display()));
        if let Some(repo) = git_toplevel(root) {
            out.push_str(&format!("- **Git top-level:** `{}`\n", repo.display()));
        }
        let cargo = root.join("Cargo.toml");
        if cargo.is_file() {
            if let Some(v) = command_version("rustc", &["--version"]) {
                out.push_str(&format!("- **rustc:** {v}\n"));
            }
        }
        if root.join("package.json").is_file() {
            if let Some(v) = command_version("node", &["--version"]) {
                out.push_str(&format!("- **node:** {v}\n"));
            }
        }
        if root.join("pyproject.toml").is_file() || root.join("requirements.txt").is_file() {
            if let Some(v) = command_version("python", &["--version"])
                .or_else(|| command_version("python3", &["--version"]))
            {
                out.push_str(&format!("- **python:** {v}\n"));
            }
        }
        if let Some(v) = command_version("git", &["--version"]) {
            out.push_str(&format!("- **git:** {v}\n"));
        }
    }

    out.push_str("\n## Resources (coarse)\n\n");
    out.push_str(&format!("- **CPU cores (logical):** {}\n", num_cpus::get()));
    if let Some(m) = memory_stats::memory_stats() {
        let mb = m.physical_mem as f64 / (1024.0 * 1024.0);
        out.push_str(&format!("- **Process RSS (approx):** {mb:.0} MiB\n"));
    }

    out.push_str("\n---\n*Cache: 5 minutes; host may invalidate on workspace switch.*\n");
    out
}

fn command_version(cmd: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(cmd).args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}
