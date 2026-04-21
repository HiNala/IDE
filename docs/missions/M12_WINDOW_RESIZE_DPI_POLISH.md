# M12 — Window, Resize, DPI: Snappy Response Polish

**Mission ID:** M12
**Prerequisites:** M11 complete. `0.2.1` packaged release is live.
**Output:** Window sizing feels *native*. Resizing a window produces zero visible hitches. DPI changes when dragging between monitors of different densities are instantaneous and crisp. Maximize, restore, and fullscreen transitions are clean. Multi-monitor setups work correctly on Windows, macOS, and Linux. The first V3 foundation: making the bones feel like a real OS-native app, not a portable game-engine demo.
**Estimated scope:** 1-2 sessions.

---

## Read First

- `/00_MISSION_INDEX.md` — standing orders.
- `/00_V3_VISION.md` — why V3 starts with foundations polish.
- `/docs/RENDERING_PIPELINE.md` — surface acquisition, format negotiation, present modes.
- `/docs/CROSS_PLATFORM.md` — per-OS window semantics.
- `/reference/03_GAPS_AND_RISKS.md` §1, §4 — DPI and cross-platform concerns.
- `https://docs.rs/winit/latest/winit/dpi/index.html` — logical vs physical coordinates.
- `https://docs.rs/wgpu/latest/wgpu/struct.Surface.html` — surface reconfiguration cost.

---

## The Situation In Plain English

The V2 editor handles resize *correctly*. M12 is about making resize feel *instant*. Right now, dragging a window edge works but you can see brief flashes — a missed frame here, a small lag there. A native Windows or macOS app doesn't do that; it resizes as if the framebuffer were painted by the OS itself. We want that feeling.

The pain points are concrete. Every resize triggers a surface reconfiguration, which is expensive because wgpu has to tear down the swapchain and rebuild it. Every resize also invalidates every layout cache in `TextLayer` (because the line count per viewport changes), forcing cosmic-text to re-shape the visible lines. And on Windows, the OS suspends paint events during a modal resize loop — meaning the window *can't* redraw until the user lets go of the edge, producing a frozen-looking rubber band. Each of these is a known problem with a known fix; M12 applies all the fixes together.

DPI is the other half. When a user drags the window between a 1x monitor and a 2x monitor, the scale factor doubles (or halves) and the entire rendering pipeline needs to re-measure everything at the new density. Done right, this happens in a single frame and looks like the window "snaps" to the new clarity. Done wrong — and we are currently in "done adequately, not done right" territory — you see a half-second of blurry text as the layout cache invalidates and re-populates.

Finally, there are the corner cases. Maximize and restore should animate smoothly. Fullscreen (F11) should be frictionless. Minimize should release GPU resources. Multi-monitor setups with mismatched DPIs, mismatched refresh rates, mismatched color spaces — all of these should Just Work. We do not get this for free; each needs explicit handling. M12 adds the handling.

---

## Scope

**In scope:**
- Resize handling that survives modal resize loops on Windows (no frozen rubber-banding).
- Pre-allocated texture & buffer pools so resize doesn't churn allocations.
- Layout cache strategy that survives resize without full invalidation (cache by `(line_index, version, wrap_width)` tuple where `wrap_width` is the only value that actually changes on resize).
- Fast DPI change: single-frame re-measurement without a visible flash.
- Smooth maximize / restore / minimize / fullscreen.
- Multi-monitor: correct behavior when window spans monitors, when monitor configuration changes, when a monitor is unplugged while a window is on it.
- Window chrome: correct title, correct icon on all three OSes.
- Refresh rate detection & adaptation: 120 Hz monitor → 120 fps rendering; 60 Hz → 60 fps.
- Battery awareness: if the OS reports "on battery," reduce frame rate ceiling to 60 fps to save power.
- New `scripts/resize-stress.ps1` automation that drags the window in a scripted pattern and captures frame timings.

**Out of scope:**
- Animated window open / close (nice-to-have, post-V3).
- Custom window decorations / title bar (huge OS-specific work, V4+).
- Transparency / vibrancy effects (post-V3).
- Picture-in-picture / always-on-top modes (post-V3).

---

## North Star

Grab the right edge of an open IDE window. Drag it around erratically for ten seconds. The content inside the window redraws fluidly through every frame. No rubber-banding. No flicker. No momentary blurriness. Now drag the window across a monitor boundary where DPI changes from 1x to 2x. The text snaps to the higher density in a single frame — no progressive re-rendering, no blur. Now hit Win+Left or Cmd+Left to snap the window. The content resizes cleanly. Everything feels like it was written for this machine, not ported to it.

---

## TODO List

### 1. Audit current resize behavior

- [ ] 1.1. Record a baseline video of resize on each OS. Identify every visible artifact: lag, flicker, rubber-band, blur, missed-frame flash.
- [ ] 1.2. Enable `latency-trace` (from M07) during resize. Measure p99 frame time during a continuous resize drag — expect it to be well over the 16 ms budget today.
- [ ] 1.3. Enumerate specific root causes in `/docs/DIAGNOSING_PERFORMANCE.md#resize`: surface reconfig cost, layout cache invalidation, Windows modal resize loop, DPI re-layout.
- [ ] 1.4. Commit: `docs(perf): document resize artifact catalog from pre-M12 baseline`.

### 2. Pre-allocate wgpu resources to avoid per-resize allocation

- [ ] 2.1. Any wgpu buffer or texture that is sized relative to viewport gets pre-allocated at a generous maximum (e.g., max expected viewport size: 7680×4320 for 8K). Resize reuses the existing allocation with updated descriptor rectangles.
- [ ] 2.2. `SurfaceManager::resize` becomes: update the `SurfaceConfiguration` width/height, call `surface.configure(&device, &config)`, but *do not* recreate any of our own textures.
- [ ] 2.3. Verify GPU memory cost: the pre-allocated over-size textures will cost more memory than needed. 8K × 4 bytes = ~130 MB for a framebuffer-sized texture. Accept this for MVP; add an adaptive approach as a V4+ follow-up if needed.
- [ ] 2.4. Unit test: 100 resizes in a row, verify no new allocations occur (use a custom allocator counter or wgpu's memory reporting).
- [ ] 2.5. Commit: `perf(render): pre-allocate viewport-sized resources to zero resize allocations`.

### 3. Fix layout cache invalidation on resize

- [ ] 3.1. The `TextLayer::layout_cache` from M04 is keyed by `(line_index, version)`. It should be `(line_index, version, wrap_width)` so that when wrap_width stays the same, a resize (width-or-height-preserving) does not invalidate.
- [ ] 3.2. If only the *height* of the viewport changes (left/right edge drag), `wrap_width` is unchanged — the cache fully survives. No re-shaping needed.
- [ ] 3.3. If the *width* changes, the cache *would* need to invalidate; but we don't soft-wrap in V3, so `wrap_width` is effectively infinite and only line length matters. Set the cache to be width-independent for non-wrapped mode.
- [ ] 3.4. Benchmark: a resize-drag scenario now completes shape work zero times per frame (everything from cache), whereas before it shaped ~55 lines per frame.
- [ ] 3.5. Commit: `perf(render): width-independent layout cache for non-wrapped mode`.

### 4. Windows: survive the modal resize loop

- [ ] 4.1. On Windows, the OS enters a modal message loop during window-edge drag — `RunMessageQueue` doesn't return to our event loop until the drag ends. This is why the window "freezes" during resize on some implementations. winit 0.30 handles this via `WindowEvent::Resized` firing repeatedly during the drag, but the render loop must be driven by those resize events, not by `RedrawRequested`.
- [ ] 4.2. In `ApplicationHandler::window_event`, when `Resized` fires, *immediately* render a frame (not via `request_redraw` which queues a redraw — actually render synchronously). This pattern is used by games and professional apps on Windows.
- [ ] 4.3. On macOS and Linux the modal loop doesn't exist; resize events interleave with redraws naturally. The synchronous-render pattern is harmless on those platforms.
- [ ] 4.4. Testing: manually drag the window edge on Windows for 10 seconds; verify content updates continuously.
- [ ] 4.5. Commit: `fix(app): render synchronously on Resized to survive Windows modal resize loop`.

### 5. Single-frame DPI change

- [ ] 5.1. `ScaleFactorChanged` fires when the window moves to a monitor of different DPI. Winit 0.30 gives both the new scale factor and a mutable reference to the inner size — it expects us to return the new size we want.
- [ ] 5.2. Compute the new physical size as `logical_size * new_scale_factor`. Call `text_layer.set_scale_factor(new_scale_factor)` which invalidates the layout cache (this is unavoidable — glyphs genuinely need re-rasterizing at new DPI).
- [ ] 5.3. Pre-rasterize the visible range *inside* the `ScaleFactorChanged` handler, before the next frame. This avoids a frame where the user sees stale glyphs.
- [ ] 5.4. Benchmark the scale factor change: on a 55-line viewport, expect ~2-4 ms for shaping + rasterization. Fits within one 16 ms frame.
- [ ] 5.5. Commit: `perf(render): eager re-rasterization on scale factor change`.

### 6. Refresh rate detection & adaptation

- [ ] 6.1. Query the monitor's refresh rate via winit's `Window::current_monitor().and_then(|m| m.refresh_rate_millihertz())`. This returns the refresh rate in thousandths of Hz (60000 = 60 Hz; 144000 = 144 Hz).
- [ ] 6.2. Choose `PresentMode::FifoRelaxed` for 60 Hz monitors (vsync with occasional tearing tolerance), `PresentMode::Mailbox` for 120+ Hz monitors (lower latency, tearing-free on compatible drivers). Fall back to `Fifo` when neither is supported.
- [ ] 6.3. When the window moves to a monitor with a different refresh rate, update the present mode. This requires a `surface.configure` call — group it with the DPI change path if they co-occur.
- [ ] 6.4. Commit: `feat(render): adaptive present mode based on monitor refresh rate`.

### 7. Battery awareness

- [ ] 7.1. Add the `battery` crate or use platform APIs: `GetSystemPowerStatus` (Windows), `IOPSCopyPowerSourcesInfo` (macOS), `/sys/class/power_supply/` (Linux). For cross-platform simplicity, `battery = "0.7"` crate works.
- [ ] 7.2. Poll battery state once every 30 seconds (not every frame — this is not a hot path). If on battery AC-disconnected: cap frame rate at 60 Hz even on 120 Hz monitors. If plugged in: uncapped.
- [ ] 7.3. Respect a user override via a config setting `power.uncap_on_battery = false` in the persisted state from M10.
- [ ] 7.4. Commit: `feat(app): cap refresh rate on battery to conserve power`.

### 8. Maximize / restore / minimize / fullscreen

- [ ] 8.1. `F11` toggles fullscreen: `window.set_fullscreen(Some(Fullscreen::Borderless(None)))` / `None`. Handle the follow-up `Resized` and `ScaleFactorChanged` events cleanly.
- [ ] 8.2. Minimize: when `Window::is_minimized()` returns true (detect via `Focused(false)` + Windows `WM_SIZE` with `SIZE_MINIMIZED`), stop requesting redraws. The buffer is effectively hidden; rendering wastes CPU and GPU. Resume on `Focused(true)` or first `Resized` event.
- [ ] 8.3. Maximize: already handled by winit implicitly; confirm no artifacts.
- [ ] 8.4. Snap (Win+Left/Right on Windows, Ctrl+Cmd+F on macOS, tiling on Linux WMs): same as resize — already handled if resize is handled correctly.
- [ ] 8.5. Commit: `feat(app): handle minimize / fullscreen / maximize state transitions cleanly`.

### 9. Multi-monitor correctness

- [ ] 9.1. When the window is dragged across monitor boundaries, `ScaleFactorChanged` fires if the monitors differ. Test: two monitors at 1x and 2x, drag slowly — each moment only one monitor "owns" the window per winit's rules; the transition is a single event.
- [ ] 9.2. When a monitor is unplugged, the window may snap to a remaining monitor. Winit fires a `Resized` and possibly a `Moved` event. Make sure the restored window state is valid (position onscreen).
- [ ] 9.3. Persist per-monitor scale factor in `PersistedState` from M10 so that reopening on the same monitor uses the right initial scale. Use the monitor's name (from winit's `MonitorHandle::name`) as a key.
- [ ] 9.4. Commit: `fix(app): multi-monitor edge cases and per-monitor scale persistence`.

### 10. Scripted resize stress test

- [ ] 10.1. `scripts/resize-stress.ps1` (Windows) and `scripts/resize-stress.sh` (unix) use `xdotool` / `WScript.Shell` / AppleScript to drive scripted mouse-drag sequences against a live editor window. Capture metrics via `--perf-smoke`-style output (the editor logs resize-frame timings to stdout when run with `--resize-telemetry`).
- [ ] 10.2. Assertions: during 10 seconds of continuous resize, p99 frame time < 32 ms (two 60 Hz frames — strict but achievable); no frame > 100 ms.
- [ ] 10.3. Add this to CI on Windows only (Linux runners don't have xdotool + Wayland, macOS scripting is fragile — Windows is the primary target anyway).
- [ ] 10.4. Commit: `test(perf): scripted resize stress test`.

### 11. Visual polish: no flash on startup

- [ ] 11.1. When the editor first opens, there's a brief moment where the window appears with the wrong size or a white/black flash. Fix: defer first `window.set_visible(true)` until *after* the first successful frame has been rendered.
- [ ] 11.2. Persisted state (from M10) already gives us target size before window creation — use it to create the window at the right size in the first place, not resize after creation.
- [ ] 11.3. Commit: `polish(app): no flash on window open`.

### 12. Icon & title bar cross-platform

- [ ] 12.1. Set the window icon on creation via `WindowAttributes::with_window_icon(Some(Icon::from_rgba(...)))` using the PNG icon assets bundled from M11.
- [ ] 12.2. On Windows, also set the small icon (used in taskbar) separately if needed; winit usually handles both.
- [ ] 12.3. Title: unchanged from M10 (`* path/to/file — IDE`) but verify it updates on every state change.
- [ ] 12.4. Commit: `polish(app): window icon on all platforms`.

### 13. Documentation

- [ ] 13.1. Update `/docs/RENDERING_PIPELINE.md` with the pre-allocated resource strategy.
- [ ] 13.2. Update `/docs/CROSS_PLATFORM.md` with the resize + DPI + multi-monitor notes.
- [ ] 13.3. Update `/docs/DIAGNOSING_PERFORMANCE.md` with the post-M12 resize characteristics.
- [ ] 13.4. Update `/docs/STATUS.md`: M12 complete, M13 next.
- [ ] 13.5. Tag: `git tag -a m12-complete -m "M12 complete: snappy resize and DPI"`; push.

### 14. Quality gates

- [ ] 14.1. `cargo fmt --all --check`.
- [ ] 14.2. `cargo clippy --workspace --all-targets --all-features -- -D warnings`.
- [ ] 14.3. `cargo test --workspace`.
- [ ] 14.4. Resize stress test passes on Windows.
- [ ] 14.5. Manual: drag window edges rapidly on each OS, cross monitor boundaries, go fullscreen, snap, minimize, maximize — no visible artifacts.
- [ ] 14.6. Performance: p50 frame time during resize < 12 ms; p99 < 32 ms; zero frames > 100 ms.
- [ ] 14.7. M08 acceptance numbers still green (static editor performance unchanged).

---

## Validation / Acceptance Criteria

1. Quality gates pass.
2. Resize video comparison shows no visible flicker or rubber-banding.
3. DPI change on monitor boundary is single-frame and crisp.
4. Fullscreen / minimize / maximize work without artifacts.
5. Multi-monitor scenarios verified on at least Windows (primary dev OS).
6. Battery cap works on a laptop.
7. `m12-complete` tag pushed.

## Testing Requirements

- Scripted resize-stress on Windows in CI.
- Manual multi-monitor verification.
- Battery cap verification on a laptop.
- All previous M08+M10 acceptance numbers preserved.

## Git Commit Strategy

10-12 commits. Push after items 3, 5, 8, 10, 13.

## Handoff to M13

M13 assumes:
- The window and rendering subsystem is production-quality.
- Adding multi-buffer state and a workspace model won't run into window-management surprises.
- All V2 baselines still hold; new work layers on top cleanly.

---

## In-repository status (engineering)

The following are implemented in code and docs (verify on your clone; some doc/script paths may need restore if the working tree hit disk limits):

- **§4 / §5 / §6 / §7 / §8 / §9 / §11 / §12:** `editor-app` — synchronous `paint_frame` on `Resized` and `ScaleFactorChanged`, `sync_present_mode`, `battery` poll + FPS cap, fullscreen (F11), skip paint when minimized, deferred `set_visible`, window icon, `monitor_scale_by_name` persistence.
- **§3 / §2 (app-owned):** `editor-render` — width-independent line shaping, `MAX_VISIBLE_ROW_SLOTS`, solid-quad fixed VB, `gpu_resize_stress` integration tests.
- **§13 (partial):** `docs/DIAGNOSING_PERFORMANCE.md` § resize artifact catalog; `docs/CROSS_PLATFORM.md`, `docs/RENDERING_PIPELINE.md` M12 notes; `docs/STATUS.md` / `MISSION_IMPLEMENTATION_STATUS.md`.
- **§10:** CI runs **`m12-gpu-resize-windows`** (`cargo test -p editor-render --test gpu_resize_stress`) for automated GPU/window resize checks; `scripts/resize-stress.*` are for **manual** drags + `--resize-telemetry` log inspection (full scripted UI drag is not in Actions).
- **§13.5 / §14.5–14.6:** `m12-complete` tag, baseline videos, and p99 tables remain **release / QA** actions.

## Standing Orders Reminder

- Pre-allocation costs memory; that's the right tradeoff for an interactive app. Never optimize memory over latency in the frame loop.
- If you fix resize but the static (non-resize) frame time regresses, you failed. Both must pass.
- Every "Just Works" claim must be manually tested on the specific OS before merge.

Go.
