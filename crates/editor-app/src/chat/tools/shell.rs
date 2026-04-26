//! Shell execution tool: run_shell (opt-in, prefix allow-list via .ide/tools.toml).

use std::path::Path;

use editor_ai_tools::ToolConfig;

/// Patterns that are never permitted regardless of the allow-list.
/// These protect the user's shell config, SSH keys, and system dirs.
const DANGEROUS_PATTERNS: &[&str] = &[
    ".bashrc", ".zshrc", ".bash_profile", ".profile", ".zprofile",
    ".ssh/", "authorized_keys", "known_hosts",
    "/etc/", "/usr/", "/bin/", "/sbin/",
    "sudoers", "passwd", "shadow",
    // Windows equivalents
    "System32", "SysWOW64", "\\Windows\\",
    "HKEY_", "reg add", "reg delete",
];

fn is_dangerous(command: &str) -> Option<&'static str> {
    let lower = command.to_lowercase();
    for pat in DANGEROUS_PATTERNS {
        if lower.contains(&pat.to_lowercase()) {
            return Some(pat);
        }
    }
    None
}

pub(super) fn tool_run_shell(input: &serde_json::Value, workspace_root: &Path) -> (String, bool) {
    let command = match input["command"].as_str() {
        Some(s) => s,
        None => return ("Missing required field 'command'".into(), true),
    };

    let config = ToolConfig::load_from_workspace_root(workspace_root).unwrap_or_default();
    if !config.shell.enabled {
        return (
            format!(
                "Shell tool is disabled. To enable it, edit {}/.ide/tools.toml:\n\n\
                 [shell]\n\
                 enabled = true\n\
                 allowed_prefixes = [\"npm\", \"npx\", \"cargo\", \"git\", \"node\", \"python\", \"tsc\", \"eslint\"]",
                workspace_root.display()
            ),
            true,
        );
    }

    let first_word = command.split_whitespace().next().unwrap_or("");
    if !config.shell.allowed_prefixes.iter().any(|p| first_word == p.as_str()) {
        return (
            format!(
                "Command prefix '{first_word}' is not in the allowed list. \
                 Add it to .ide/tools.toml under [shell] allowed_prefixes."
            ),
            true,
        );
    }

    // Secondary safety check: block writes to sensitive paths regardless of allow-list.
    if let Some(pat) = is_dangerous(command) {
        return (
            format!(
                "Command blocked: contains sensitive pattern '{pat}'. \
                 Shell commands must not target shell configs, SSH keys, or system directories."
            ),
            true,
        );
    }

    let cwd = if let Some(d) = input["cwd"].as_str() {
        workspace_root.join(d)
    } else {
        workspace_root.to_path_buf()
    };

    let (shell, flag) = if cfg!(windows) { ("cmd", "/C") } else { ("sh", "-c") };

    match std::process::Command::new(shell).arg(flag).arg(command).current_dir(&cwd).output() {
        Err(e) => (format!("Failed to start shell: {e}"), true),
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            let stderr = String::from_utf8_lossy(&out.stderr);
            let combined = format!(
                "exit_code: {}\nstdout:\n{}\nstderr:\n{}",
                out.status.code().unwrap_or(-1),
                &stdout[..stdout.len().min(50_000)],
                &stderr[..stderr.len().min(10_000)],
            );
            (combined, !out.status.success())
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn tmp() -> TempDir { tempfile::tempdir().unwrap() }

    fn enabled_workspace(d: &TempDir, prefixes: &[&str]) {
        let ide = d.path().join(".ide");
        std::fs::create_dir_all(&ide).unwrap();
        let prefix_toml: String = prefixes.iter()
            .map(|p| format!("\"{p}\""))
            .collect::<Vec<_>>()
            .join(", ");
        std::fs::write(
            ide.join("tools.toml"),
            format!("[shell]\nenabled = true\nallowed_prefixes = [{prefix_toml}]\n"),
        ).unwrap();
    }

    #[test]
    fn disabled_by_default() {
        let d = tmp();
        let (out, err) = tool_run_shell(
            &serde_json::json!({"command": "echo hi"}),
            d.path(),
        );
        assert!(err, "{out}");
        assert!(out.contains("disabled"), "{out}");
    }

    #[test]
    fn disallowed_prefix_rejected() {
        let d = tmp();
        enabled_workspace(&d, &["npm"]);
        let (out, err) = tool_run_shell(&serde_json::json!({"command":"rm -rf /"}), d.path());
        assert!(err, "{out}");
        assert!(out.contains("not in the allowed list"), "{out}");
    }

    #[test]
    fn dangerous_pattern_blocked_even_when_allowed() {
        let d = tmp();
        enabled_workspace(&d, &["echo"]);
        let (out, err) = tool_run_shell(
            &serde_json::json!({"command": "echo foo >> ~/.bashrc"}),
            d.path(),
        );
        assert!(err, "{out}");
        assert!(out.contains("blocked"), "{out}");
    }

    #[test]
    fn ssh_key_write_blocked() {
        let d = tmp();
        enabled_workspace(&d, &["cat"]);
        let (out, err) = tool_run_shell(
            &serde_json::json!({"command": "cat id_rsa.pub >> ~/.ssh/authorized_keys"}),
            d.path(),
        );
        assert!(err, "{out}");
        assert!(out.contains("blocked"), "{out}");
    }

    #[test]
    fn echo_succeeds_when_allowed() {
        let d = tmp();
        enabled_workspace(&d, &["echo"]);
        let (out, err) = tool_run_shell(
            &serde_json::json!({"command": "echo antigravity_test"}),
            d.path(),
        );
        assert!(!err, "{out}");
        assert!(out.contains("antigravity_test"), "{out}");
    }
}
