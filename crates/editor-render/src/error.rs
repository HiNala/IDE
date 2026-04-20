//! GPU and surface initialization errors for `editor-render`.

use thiserror::Error;

/// Errors from `wgpu` surface setup, adapter selection, or device creation.
#[derive(Debug, Error)]
pub enum RenderError {
    /// No adapter matched the requested backends / surface compatibility.
    #[error("no suitable GPU adapter found")]
    NoAdapter,
    /// Surface could not be created for the given window.
    #[error("failed to create surface: {0}")]
    SurfaceCreate(String),
    /// Adapter enumeration failed.
    #[error(transparent)]
    RequestAdapter(#[from] wgpu::RequestAdapterError),
    /// Async device request failed.
    #[error(transparent)]
    RequestDevice(#[from] wgpu::RequestDeviceError),
    /// Surface could not provide a drawable texture (lost, timed out, etc.).
    #[error("no drawable surface texture: {0}")]
    SurfaceTexture(&'static str),
}
