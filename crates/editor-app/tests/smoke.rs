//! Integration smoke: `--dry-run` exercises headless `wgpu` init (CI-friendly).

use std::process::Command;

#[test]
fn dry_run_exits_zero() {
    let exe = env!("CARGO_BIN_EXE_editor-app");
    let status = Command::new(exe)
        .arg("--dry-run")
        .status()
        .expect("spawn editor-app --dry-run");
    assert!(status.success(), "dry-run status: {status}");
}
