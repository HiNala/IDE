//! Backend bitmask selection (M03): platform defaults when `WGPU_BACKEND` is unset.

/// Returns which wgpu backends the [`wgpu::Instance`] should enable.
///
/// If the `WGPU_BACKEND` environment variable is set (see [`wgpu::Backends::from_env`]),
/// that value wins. Otherwise we use platform defaults aligned with the mission:
/// DirectX 12 + Vulkan (+ GL) on Windows, Metal on macOS, Vulkan + GL elsewhere.
#[must_use]
pub fn instance_backends() -> wgpu::Backends {
    if let Some(bits) = wgpu::Backends::from_env() {
        return bits;
    }
    #[cfg(target_os = "windows")]
    {
        wgpu::Backends::DX12 | wgpu::Backends::VULKAN | wgpu::Backends::GL
    }
    #[cfg(target_os = "macos")]
    {
        wgpu::Backends::METAL
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        wgpu::Backends::VULKAN | wgpu::Backends::GL
    }
}

#[cfg(test)]
mod tests {
    use super::instance_backends;

    #[test]
    fn instance_backends_is_non_empty() {
        assert_ne!(instance_backends(), wgpu::Backends::empty());
    }
}
