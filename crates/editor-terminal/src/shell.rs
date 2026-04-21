//! Detect default shell per platform.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::error::TerminalError;

/// Executable + args + optional extra environment for the PTY child.
#[derive(Debug, Clone)]
pub struct ShellConfig {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub env: HashMap<String, String>,
}

impl ShellConfig {
    /// Single executable, no args (interactive login behavior depends on the shell).
    pub fn from_program(program: PathBuf) -> Self {
        Self { program, args: Vec::new(), env: HashMap::new() }
    }
}

fn which_on_path(cmd: &str) -> Option<PathBuf> {
    let Ok(paths) = std::env::var("PATH") else {
        return None;
    };
    let sep = if cfg!(windows) { ';' } else { ':' };
    for dir in paths.split(sep) {
        let p = Path::new(dir).join(cmd);
        if p.is_file() {
            return Some(p);
        }
    }
    None
}

/// Resolve the user's shell: optional override, else platform default.
pub fn detect_shell(
    override_shell: Option<&std::ffi::OsStr>,
) -> Result<ShellConfig, TerminalError> {
    if let Some(p) = override_shell {
        if p.is_empty() {
            return default_shell();
        }
        let pb = PathBuf::from(p);
        return Ok(ShellConfig::from_program(pb));
    }
    default_shell()
}

fn default_shell() -> Result<ShellConfig, TerminalError> {
    if cfg!(unix) {
        if let Ok(s) = std::env::var("SHELL") {
            if !s.is_empty() {
                return Ok(ShellConfig::from_program(PathBuf::from(s)));
            }
        }
        return Ok(ShellConfig::from_program(PathBuf::from("/bin/sh")));
    }

    if cfg!(windows) {
        if let Some(pwsh) = which_on_path("pwsh.exe") {
            return Ok(ShellConfig::from_program(pwsh));
        }
        if let Some(ps) = which_on_path("powershell.exe") {
            return Ok(ShellConfig::from_program(ps));
        }
        if let Ok(comspec) = std::env::var("COMSPEC") {
            if !comspec.is_empty() {
                return Ok(ShellConfig::from_program(PathBuf::from(comspec)));
            }
        }
        return Ok(ShellConfig::from_program(PathBuf::from(r"C:\Windows\System32\cmd.exe")));
    }

    Ok(ShellConfig::from_program(PathBuf::from("/bin/sh")))
}
