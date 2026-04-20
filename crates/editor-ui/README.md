# `editor-ui`

Minimal UI layer: gutter (line numbers in V2), status bar, and other non-text
chrome. Must not depend on `wgpu` or `winit`; it consumes state from
`editor-core` and passes draw intents to `editor-render` in later missions.
