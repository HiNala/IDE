//! `editor-app` — binary shell for the IDE project.
//!
//! In M01 this is a boot banner: we initialize `tracing`, verify that every
//! workspace member was linked in, and exit cleanly. Later missions grow it:
//!
//! - **M03:** owns the `winit` event loop and `wgpu` `Surface`.
//! - **M05:** drives the input → state → render frame pipeline.
//! - **M06:** wires file I/O into the main loop over a bounded channel.
//! - **M07:** registers the dev overlay under the `dev-overlay` feature.
//!
//! See `ARCHITECTURE.md` for the wiring and `docs/MISSIONS.md` for the plan.

#![forbid(unsafe_code)]

use anyhow::Result;
use tracing::info;
use tracing_subscriber::EnvFilter;

/// Crate version string, sourced from `Cargo.toml` at compile time.
const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() -> Result<()> {
    init_tracing();

    info!(version = VERSION, "ide: starting");
    info!("linked subsystems:");
    info!("  {}", editor_core::banner());
    info!("  {}", editor_render::banner());
    info!("  {}", editor_input::banner());
    info!("  {}", editor_io::banner());
    info!("ide: M01 scaffold boot complete; exiting (no window yet — M03).");

    Ok(())
}

/// Initialize `tracing_subscriber` with a sensible default filter and a
/// human-readable formatter. Honors `RUST_LOG` when set.
fn init_tracing() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,editor_app=info,editor_core=info"));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(true)
        .with_level(true)
        .compact()
        .init();
}
