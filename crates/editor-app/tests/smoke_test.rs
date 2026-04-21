// Integration smoke: `--dry-run` exercises headless `wgpu` init (CI-friendly).

use std::path::{Path, PathBuf};
use std::process::Command;

fn editor_app_exe() -> PathBuf {
    if let Some(p) = std::env::var_os("CARGO_BIN_EXE_editor_app") {
        return PathBuf::from(p);
    }
    // `cargo test` normally sets the above; fall back for odd runners / tooling.
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    for profile in ["debug", "release"] {
        let name = if cfg!(windows) { "editor-app.exe" } else { "editor-app" };
        let candidate = manifest_dir.join("../../target").join(profile).join(name);
        if candidate.is_file() {
            return candidate;
        }
    }
    panic!(
        "could not find editor-app binary (set CARGO_BIN_EXE_editor_app or build with cargo test); tried next to {manifest_dir:?}"
    );
}

#[test]
fn dry_run_exits_zero() {
    let exe = editor_app_exe();
    let status = Command::new(exe).arg("--dry-run").status().expect("spawn editor-app --dry-run");
    assert!(status.success(), "dry-run status: {status}");
}
