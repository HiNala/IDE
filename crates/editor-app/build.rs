//! Embeds Windows application manifest (long paths + UTF-8 code page).
//!
//! See [`docs/CROSS_PLATFORM.md`](../../docs/CROSS_PLATFORM.md) for rationale.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(windows)]
    {
        let mut res = winres::WindowsResource::new();
        res.set_manifest_file("windows/app.manifest");
        res.compile()?;
    }
    Ok(())
}
