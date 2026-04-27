[← docs/](./) · [README](../README.md) · [PRD](./PRD.md) · [ARCHITECTURE](./ARCHITECTURE.md)

# UI Strategy — Stay Native, Fix the Layout

**Status:** In progress (M30 — shell metrics + taffy status bar; full `paint_tree` TBD).
**Authors:** IDE core team.
**Last updated:** 2026-04-27.

## Execution checklist (28 items)

Track in-repo; check off as you merge. *Dependency:* `taffy` is in `[workspace.dependencies]` (**0.10.x**; older “0.5” mentions in §6 are stale).

### M29 — Layout foundation

- [x] **1.** Add `crates/editor-layout` + workspace `taffy`.
- [x] **2.** `editor-layout` crate: `#![forbid(unsafe_code)]`, no `wgpu` / `winit`.
- [x] **3.** `LayoutRect`, `LayoutItem`, `LayoutResult`.
- [x] **4.** `LayoutEngine` wrapping `TaffyTree` (`new_leaf` / `new_with_children`).
- [x] **5.** `LayoutEngine::compute` + `result` → bounds.
- [x] **6.** Unit test: flex row three children; middle = remainder at 800px.
- [x] **7.** `TextMeasure` + `MonospaceWidthMeasure` stub (real measure in `editor-render` TBD).

### M30 — Chrome layout + `paint_tree`

- [x] **8.** Pure shell layout API: `editor_layout::chrome_shell` — `MainChromeParams` / `MainChromeLayout` / `compute_main_chrome_layout`, alias `build_chrome_tree`; re-exported from `editor_ui::chrome_layout`.
- [x] **8b.** `main_chrome_to_layout_result` + `ChromeWidgetId` for flat `LayoutItem` hit regions (replaces ad-hoc offsets incrementally).
- [ ] **9.** `editor_ui::paint_from_layout` / `paint_tree`: walk **full** `LayoutResult` (or taffy tree) → all `FrameChrome` quads + lines. *(Status bar sub-path done: taffy row + 3 glyphon buffers + `append_status_bar_text_areas`.)*
- [x] **10.** Status bar: left / center (ellipsis) / right + `layout_status_bar_row` / `StatusBarLayout`.
- [ ] **11.** `title_bar` (inline `lib.rs` module) → layout nodes or measured runs.
- [ ] **12.** `tab_strip` + `breadcrumbs` → taffy nodes; keep `TabHit` / `BreadcrumbHit` in sync. *(Ellipsis + `MainChromeLayout` strip width done.)*
- [ ] **13.** `sidebar` + `agent_panel` + `terminal_header` → layout nodes. *(Chips: label ellipsize; min/max agent width in theme.)*
- [x] **14a.** `build_frame_chrome` uses `compute_main_chrome_layout` for all outer shell metrics (insets, agent rail, strip width, `content_bottom`).  
- [x] **14b.** Cache `LayoutResult` on the app (`shell_layout`) each `build_frame_chrome` for hit testing; full pointer routing TBD.
- [x] **15.** `crates/editor-ui/tests/layout_snapshot.rs` + `tests/snapshots/main_chrome_*.txt` goldens (960 / 1920 / 2560) via `format_main_chrome_layout_golden`.
- [ ] **16.** Shrink `main.rs` chrome path (target **−600–800 LOC** by end of M30) after `paint_tree` migration.

### M31 — Clipping, scrollbar, overflow

- [ ] **17.** `editor-render`: `set_scissor_rect` per panel (editor body, bottom terminal, agent transcript/composer). *(Chrome [`FrameChrome`] text uses per-line [`ChromeTextLine::clip`] → glyphon `TextBounds` today.)*
- [ ] **18.** `UiOverflow` (`Clip` | `Ellipsis` | `Visible`) on layout leaves; default `Clip` for panels, `Ellipsis` for one-line labels.
- [ ] **19.** Scrollbar: geometry from `LayoutResult` / `MainChromeLayout`; keep `ScrollbarInput` behavior.
- [ ] **20.** Acceptance: long tab / breadcrumb / status / chip strings stay inside rects (GPU clip + CPU ellipsis).

### M32 — Cursor parity + perf

- [x] **21a.** §5.1 palette + spacing: `STATUS_BAR_BG`, `AGENT_PANEL_BG`, `AGENT_COMPOSER_BG`, `HAIRLINE`; tab 32, sidebar 220, agent 360 (320–640).  
- [x] **21b.** `AGENT_TRANSCRIPT_BG`, `AGENT_TERMINAL_BG` in palette; `CONTEXT_CHIP_HEIGHT` / `CONTEXT_CHIP_GAP` in `theme::spacing` (transcript fill uses transcript token).
- [x] **22.** `visual_smoke` doc + optional `docs/assets/cursor_style_reference.png` + `editor-ui` `reference_visual` test (pixel diff TBD).
- [ ] **23.** Title bar: Windows drag hit-region; `Ctrl+P` from search pill.
- [ ] **24.** Agent composer: max 8 lines, `Enter` / `Shift+Enter`, model popover from layout.
- [ ] **25.** Criterion benches: shell layout + taffy + `text_layer::measure` vs §7.
- [ ] **26.** LRU `text_layer` measure cache + dev hit-rate log.
- [ ] **27.** `MISSION_IMPLEMENTATION_STATUS.md` + briefs M29–M32; CI `fmt` / `clippy` / `test` all targets.
- [ ] **28.** **Exit review** (§9): if M30 stalls, document `gpui-ce` or hybrid per triggers.

> TL;DR — The "cut off at the edges" problem is **not caused by wgpu or glyphon**. It
> is caused by the current UI having no layout engine and no text-measurement step:
> chrome paints at hand-computed pixel offsets, and the status bar is a single
> pre-formatted string that runs off the right edge when it gets long. We fix
> that by adopting [`taffy`](https://docs.rs/taffy) for flexbox layout,
> routing text through a measurement pass before paint, and clipping every
> surface to its own rect. This preserves the PRD's "no Electron, no VM, no
> embedded browser" constraint and the sub-5 ms input-to-pixel budget. A
> hard reassessment is scheduled at the end of M29 with measurable exit
> criteria; if we fail them, we switch to the [`gpui-ce`](https://github.com/gpui-ce/gpui-ce)
> community fork (the most similar runtime to what we already have) rather
> than a web-based shell.

---

## 1. Problem statement

The current shell renders beautifully in isolation but breaks visually on common
window sizes:

- The status bar string (`0 errors · 0 warnings · untitled · Ln 5, Col 1 · 5 lines · UTF-8 · LF · IDE - M21 ★`)
  is built once in `StatusBarLayout::from_info` and rendered through a single
  `glyphon::Buffer` the full width of the window. There is **no split into
  left/right clusters**, **no ellipsization**, and **no measurement** of the
  string before paint, so whatever is at the right edge gets clipped by the
  window.
- Chrome panels (sidebar, tab strip, agent panel, breadcrumbs, terminal header)
  are painted at hand-computed pixel offsets in `build_frame_chrome` in
  `crates/editor-app/src/main.rs` (~4 200 LOC). Every time we add a new strip
  (e.g. the title bar) we have to shift every downstream offset by hand and
  re-check every mouse-hit region. That is why recent commits kept breaking
  the click map.
- `agent_panel.paint` lays out its rows by repeatedly adding logical pixel
  constants (`PANEL_INNER_PAD`, `ROW_LINE_HEIGHT`, …). When the panel's width
  falls below the sum of a chip's dot + label + close affordance, the close
  icon jumps outside the chip rect. No clipping mask prevents that.
- There is no generic "measure text → pick font size / truncate / ellipsize"
  path. We only ask `glyphon` to shape what we already decided to draw.

**Diagnosis:** the pain is immediate-mode-with-no-layout-engine. Rust and wgpu
are not to blame. Zed (which ships a full IDE using its own GPU framework)
works around exactly this problem by having a layout tree and a measurement
pass. We do not yet.

## 2. Goals, non-goals, and binding constraints

### Goals

1. No cut-off strings on any window size ≥ 960×600 @ 1.0 scale, up to 4K @ 2.0
   scale. Every text surface either fits or gracefully ellipsizes.
2. Pixel-accurate Cursor-style shell: title bar, sidebar, tab strip,
   breadcrumbs, editor, agent panel with session tabs / context chips /
   transcript / composer / embedded terminal, status bar.
3. Keep the acceptance budgets from `docs/PERFORMANCE_BUDGETS.md`:
   - cold start < 1 s,
   - input-to-pixel < 5 ms,
   - steady-state frame < 16 ms (target 8 ms),
   - bounded memory.
4. Hover, focus, and mouse hit-testing stay correct when chrome reflows.
5. Moves the `editor-app/src/main.rs` paint loop below 2 500 LOC by the end of
   the programme (it is ~4 200 now and most of the bloat is inline chrome).

### Non-goals

- Building a general-purpose GUI framework. We only solve what this IDE needs.
- Adding full HTML/CSS. Flexbox via `taffy` is sufficient.
- Soft-wrap editing or rich-text rendering in the document itself (those are
  editor-core concerns governed by existing missions).

### Binding constraints (from `docs/PRD.md` §2)

- **No Electron, no VM, no embedded browser** at runtime. This rules
  `Tauri` / `Electron` / `Dioxus-Desktop (wry-webview)` out as the primary
  shell, even though they are the easiest path to a Cursor-style UI.
- **GPU-accelerated** via `wgpu` (DX12 / Metal / Vulkan / GL fallback).
- **Sub-5 ms input-to-pixel latency.** This is violated by any shell that
  renders through a web content process on the main input path.
- **Deterministic behavior** — identical inputs produce identical pixels. This
  favours a retained layout tree over our current procedural paint.

These three constraints together collapse the decision space.

## 3. Options considered (2026 survey)

| Option | What we would do | Verdict |
|---|---|---|
| **A. Stay on `wgpu + glyphon`, add `taffy` layout + text measurement.** | Keep the existing crates. Introduce a `UiNode` tree backed by `taffy::TaffyTree`, a `FrameChrome` painter that walks the computed tree, and a single text-measurement cache. Ellipsize/flex everywhere. | **Recommended.** Preserves every PRD constraint, keeps ~19 crates of investment, fixes the root cause. Medium-sized refactor (~2-3 weeks). |
| **B. Port the shell to `gpui` (community fork `gpui-ce`).** | Replace `editor-ui` + most of `paint_frame` with GPUI views. Keep `editor-core`, `editor-io`, `editor-render` (text canvas), `editor-chat`, `editor-workspace`, `editor-ai-*` untouched. | **Fallback.** Same rendering model as today (GPU-native, flexbox layout, Rust-only), proven on Zed. But GPUI is pre-1.0, Zed Industries paused active development in late 2025, and the community fork is small. Big rewrite of the chrome for uncertain maintenance. |
| **C. Port to `iced` or `egui`.** | Rewrite `editor-ui` against one of these. Keep the rest. | **Rejected.** `iced` does not meet the latency budget for the editor surface; `egui` is immediate-mode and struggles on dense IDE chrome. Neither gives us the level of visual control we need for a Cursor-style look. |
| **D. Tauri 2 + React (or Solid) front-end; Rust back-end via `invoke`.** | Move chrome to a webview; keep the text canvas either in a `<canvas>` bound to `wgpu` (via `raw-window-handle` into a child window) or inside an Electron-style webview. | **Rejected for the shell.** Violates the PRD's "no embedded browser" rule and busts the input-latency budget. *Retained as a fallback **only** if Option A stalls for schedule reasons; see §9.* |
| **E. `Slint` or `Xilem`.** | Rewrite chrome in the declarative DSL. | **Rejected.** Slint's text performance and custom-widget story are not proven for an IDE canvas. Xilem is still research-grade. |

Rationale sources: current `docs/PRD.md` §2-4, current `docs/PERFORMANCE_BUDGETS.md`,
2026 surveys of the Rust GUI ecosystem, Zed's own [decision to pause GPUI
development](https://news.ycombinator.com/item?id=47003569) and the community
fork at [`gpui-ce/gpui-ce`](https://github.com/gpui-ce/gpui-ce), and a
`Tauri` / `Electron` 2026 benchmark survey (Tauri 2 is ~96 % smaller than
Electron but still costs an OS webview process per window — acceptable for
_some_ apps, not for our input budget).

## 4. Chosen plan — Option A, in four layers

We keep the existing crate boundary from `docs/ARCHITECTURE.md`. We add one
new crate, `editor-layout`, and we refactor `editor-ui` and a thin slice of
`editor-render` to consume it.

### 4.1 Crate diff

```text
editor-layout  NEW
    └── taffy 0.10             # flexbox/grid engine (crates.io current)
    └── re-exports:            UiNode, UiTree, UiRect, LayoutResult,
                               Measure, Ellipsize

editor-ui      UPDATED
    ├── (existing paint modules become builders that emit UiNode trees
    │    rather than emitting ChromeQuads directly)
    ├── theme.rs               # unchanged — palette + spacing tokens
    ├── chrome.rs              # gains `UiPainter::paint_tree(&LayoutResult)`
    └── measure.rs  NEW        # wraps glyphon measurement into a text cache

editor-render  UPDATED
    └── text_layer.rs          # exposes `measure(text, font, scale) -> Extents`
                                 so `editor-layout` can ask cosmic-text what a
                                 string's pixel width would be **without**
                                 actually shaping it for paint.

editor-app     UPDATED
    └── main.rs                # paint_frame becomes:
                                   build_tree → layout.compute → paint
                                 No more hand-computed offsets.
```

No dependency arrow reverses: `editor-layout` does **not** see `wgpu` or
`winit`. It is a pure function from `UiNode` tree + available space to
`(rect, text-run)` pairs.

### 4.2 Data-flow per frame

```text
  app state              app state + window size
      │                           │
      ▼                           ▼
  build_chrome_tree() ─────► taffy::compute()
        (pure)                    │
                                  ▼
                        LayoutResult = {rects, text-runs, hit-regions}
                                  │
                                  ├─► UiPainter   → FrameChrome quads/lines
                                  ├─► HitBuilder  → hit-regions (tabs, chips,
                                  │                  title icons, breadcrumbs)
                                  └─► editor-render feeds text runs through
                                      the existing glyphon path.
```

This is the same shape as every retained-mode GUI (Flutter, GPUI, Yoga/RN,
Blink). We are not inventing anything new; we are catching up to the layout
step we skipped in the MVP.

### 4.3 Text-measurement cache

The new bug class we have to avoid is "measure 500 strings per frame and
blow the CPU budget". `editor-render::text_layer::measure` will cache
`(font, scale, text)` → `Extents` in an `LruCache` sized to the visible
widget count (low hundreds). Cache hit ≈ O(1) pointer chase; cache miss
shapes with cosmic-text. We already do the expensive shaping per visible
text-run; measurement is strictly cheaper than shape-and-paint.

### 4.4 Clipping

Every panel gets its own scissor rect pushed via `wgpu::RenderPass::set_scissor_rect`
keyed off the laid-out node's rect. Text that overflows is clipped at the GPU,
not at the CPU. The `taffy` layout already ellipsizes per-node when we ask
(via a `text_overflow: ellipsis` flag on leaf text nodes). So:

- Status-bar left cluster shrinks with `…` when the branch name is very long.
- Tab labels ellipsize when their flex box under-allocates.
- Agent context chip labels ellipsize before the close `×` icon.
- Window chrome never visually leaks across panel borders.

### 4.5 Hit-testing

`LayoutResult` carries a parallel `Vec<HitRegion>`. Mouse events consult it
once per event — **no more walking hand-written offsets** in `main.rs`. This
collapses ~600 LOC of branchy offset math into a flat lookup.

### 4.6 What does NOT change

- `editor-core` (rope, cursor, selection, undo, worker pool).
- `editor-io`, `editor-workspace`, `editor-git`, `editor-search`,
  `editor-index`, `editor-diff`, `editor-syntax`, `editor-terminal`.
- `editor-ai-provider`, `editor-ai-tools`, `editor-chat`, `editor-metadata`,
  `editor-skills`, `editor-settings`.
- `editor-render::editor_renderer` (frame-loop entry, GPU context,
  text-layer shape/draw).
- `wgpu`, `winit`, `glyphon`, `cosmic-text` versions.

This scopes the refactor to the chrome. The expensive editor hot path is
untouched.

## 5. Visual target — matches the user-supplied Cursor-style reference

The reference screenshot prescribes this concrete layout (all dimensions in
**logical** px, scaled by `scale_factor` at paint time). These numbers
become the defaults for the `editor-layout` nodes and will ship as
tokens in `editor-ui::theme::spacing`.

```text
┌─ title_bar (34) ─────────────────────────────────────────────────────┐
│ IDE | File Edit View Go Run    [ 🔍 Search or jump to file… ]   ▣⎙⚙👤 │
├─ sidebar ─┬─ tabstrip (32) ───────────────────┬─ agent header (32) ──┤
│   FILES   │ ● sidecar.rs  • entry.rs  • M21…  │ M21 Sidecar  ▸ tabs  │
│           ├─ breadcrumbs (24) ────────────────┼──────────────────────┤
│           │ crates / editor-metadata / src /  │ CONTEXT              │
│   crates  │            sidecar.rs             │ ● sidecar.rs × ● …   │
│   docs    ├─────────────────────────────────── │ ──────────────────── │
│   Cargo…  │                                   │ (transcript scroll)  │
│           │   1  // crates/editor-metadata/…  │                      │
│           │   2                                │                      │
│           │   3  use std::path::…             │                      │
│           │                                    │ ──────────────────── │
│           │         (editor canvas)           │ [ composer textarea ]│
│           │                                    │  model · … · Send ✈ │
│           │                                    ├──────────────────────┤
│           │                                    │ Terminal|Problems|Out│
│           │                                    │   Compiling…        │
│           │                                    │   Finished dev…     │
├────────────────────────────────────────────────┴──────────────────────┤
│ ⎇main* · 0 errors · 2 warnings  …   Ln 47,Col 1 · UTF-8 · Rust · IDE★ │
└───────────────────────────────────────────────────────────────────────┘
```

### 5.1 Token changes landing with this plan

- `TITLE_BAR_HEIGHT = 34`
- `TAB_STRIP_HEIGHT = 32` (was 28)
- `BREADCRUMBS_HEIGHT = 24`
- `STATUS_BAR_HEIGHT = 24`
- `TERMINAL_HEADER_HEIGHT = 28`
- `AGENT_PANEL_MIN_WIDTH = 320`
- `AGENT_PANEL_MAX_WIDTH = 640`
- `AGENT_PANEL_DEFAULT_WIDTH = 360`
- `SIDEBAR_DEFAULT_WIDTH = 220`
- `CONTEXT_CHIP_HEIGHT = 22`
- `CONTEXT_CHIP_GAP = 8`
- Status-bar layout: `grid-template-columns: min-content 1fr min-content`;
  middle cluster **ellipsizes**, not the outer ones.

### 5.2 Palette tightened to match the reference

The `editor-ui::theme::palette` changes kept from the last round
(`ACCENT_BLUE = #A855F7`, `EDITOR_BG = #08080c`, `TAB_STRIP_BG = #0c0c14`,
etc.) remain. We add:

- `STATUS_BAR_BG = #080810` (was same as `TAB_STRIP_BG`; separate now).
- `AGENT_PANEL_BG = #0a0a14`.
- `AGENT_TRANSCRIPT_BG = #0b0b14`.
- `AGENT_COMPOSER_BG = #13131c`.
- `AGENT_TERMINAL_BG = #0a0a10`.
- `HAIRLINE = rgba(255,255,255,0.06)` — used for every internal border.

## 6. Sub-missions (M29 → M32)

Each mission ships end-to-end, compiles green, passes `cargo fmt/clippy/test`
on Windows (primary), macOS, Linux, and lands behind no feature flag. None
of them changes crate names or the public surface of `editor-core`.

### M29 — `editor-layout` crate (taffy bring-up)

- **Add** `crates/editor-layout` as a new workspace member.
- **Add** `taffy = "0.5"` to workspace deps.
- **Expose** `UiNode`, `UiTree`, `UiLayout`, `compute(root, space)` that
  returns `Vec<(node_id, Rect)>`. Wrap `taffy::Style` so callers never see
  taffy types directly.
- **Provide** a `Measure` trait for leaf text nodes: given
  `(FontKey, FontSize, Text, MaxWidthPx)` returns `(WidthPx, HeightPx,
  truncated: bool)`. `editor-render::text_layer::measure` implements it.
- **Unit tests**: flex row with 3 children shrinks in the middle;
  ellipsization flag is set; right-column icons never move left of their
  `min-content`.
- **Acceptance**: `cargo test -p editor-layout` green on CI; no dependency
  on `wgpu` / `winit`.

### M30 — Port chrome to `editor-layout`

- **Rewrite** `sidebar.rs`, `tab_strip.rs`, `breadcrumbs.rs`,
  `status_bar.rs`, `title_bar`, `terminal_header.rs`, `agent_panel.rs` as
  "builders" that produce a `UiNode` tree instead of painting directly.
- **Add** `editor-ui::paint_tree(layout: &UiLayout, chrome: &mut FrameChrome)`:
  one walk that pushes quads and text lines. This replaces every inline
  `push_quad` call in chrome modules.
- **Rewrite** `main.rs::build_frame_chrome` as:
  1. build tree (pure, no GPU),
  2. `editor_layout::compute(root, window_rect)`,
  3. `editor_ui::paint_tree(layout, chrome)`,
  4. extract hit regions from layout by node id.
- **Delete** the hand-computed offset math in `paint_frame`. `main.rs`
  should drop ~600-800 LOC.
- **Acceptance**:
  - status bar ellipsizes in the middle cluster at 800 px width;
  - agent panel chips ellipsize the label, never the dot or the `×`;
  - tabs ellipsize their label, never their modified-dot;
  - resizing between 960 px and 2 560 px never produces clipped chrome at
    integer scale factors 1.0 / 1.5 / 2.0.
- A new `tests/layout_snapshot.rs` in `editor-ui` exercises the tree at
  three window sizes and diffs against a recorded JSON (think `insta`).

### M31 — Editor canvas clipping + scrollbar polish

- **Add** scissor rects per panel in `editor-render::editor_renderer`
  (one for the editor viewport, one for the terminal pane, one for the
  agent transcript). Text that would overflow their node rect is
  clipped at the GPU, not the CPU.
- **Rewrite** the vertical scrollbar as an `editor-layout` node inside
  the editor panel. Drag math stays in `editor-ui::scrollbar` but its
  geometry comes from the layout.
- **Add** a `UiOverflow::Clip | Ellipsis | Visible` per leaf; default
  `Clip` for panel roots and `Ellipsis` for text lines in chrome.
- **Acceptance**: long tab names, long breadcrumb paths, long status-bar
  strings, and long context-chip labels all visibly render with "…" and
  never leak across a panel border.

### M32 — Polish & Cursor-style parity pass

- **Re-theme** to the §5 tokens. Diff the rendered UI against the
  user-supplied reference using `visual_smoke.rs`.
- **Rebuild** the title bar with the real top-right icon cluster
  (explorer / settings / chat / user), draggable window region for
  Windows platforms, and the centered search pill with `Ctrl+P`
  activation.
- **Fix** the agent composer:
  - multi-line grow up to 8 lines then scroll,
  - `Enter` sends, `Shift+Enter` newline,
  - model dropdown becomes a real popover (positioned by layout).
- **Acceptance**: run the app at 1366×768, 1920×1080, 2560×1440 — all
  three match the reference within ~4 px on panel boundaries; no text
  leaks; input-to-pixel latency budget unchanged (measured with the
  existing perf-smoke harness).

## 7. Performance budget and measurement

| Phase | Current | After M32 target | Guardrail |
|---|---|---|---|
| `build_chrome_tree`     | n/a        | ≤ 0.3 ms | criterion bench in `editor-ui` |
| `taffy::compute`        | n/a        | ≤ 0.4 ms | criterion bench in `editor-layout` |
| `paint_tree`            | ~0.5 ms    | ≤ 0.5 ms | no regression |
| `text_layer::measure`   | n/a        | ≤ 0.2 ms | LRU hit rate > 0.9 |
| `prepare + gpu`         | <8 ms p95  | <8 ms p95 | existing `frame_overhead.rs` |
| Input-to-pixel          | <5 ms p95  | <5 ms p95 | `input_hotpath.rs` + perf-smoke |
| Cold start              | <1 s       | <1 s | unchanged |

The `taffy` layer costs < 1 ms per frame on any desktop CPU for the node
counts we care about (≤ 300). This is well inside the 16 ms frame budget
and leaves the hot editor path untouched.

## 8. Testing strategy

- **Layout snapshot tests** (`editor-ui/tests/layout_snapshot.rs`): serialize
  the computed layout to JSON at 960×600, 1920×1080, 2560×1440, compare
  against checked-in goldens.
- **Visual smoke** (`editor-render/tests/visual_smoke.rs`): already exists;
  add reference frames matching the user's Cursor-style screenshot.
- **Property tests** on ellipsize invariants: for any text `s` and any
  width `w`, the laid-out text width ≤ `w` and if truncated ends with
  `'…'`. Already easy with `proptest`.
- **Hit-test fuzz**: random clicks at random window sizes; the click
  dispatch function must always land inside exactly one of
  `{sidebar, tab_strip, breadcrumbs, editor, scrollbar, agent_panel,
  terminal, status_bar}` or outside the window.

## 9. Exit criteria & fallback plan

We commit **now** to Option A. We will re-open the decision **only** if,
at the end of M30, any of the following is true:

1. Port of chrome to `editor-layout` exceeds **2 calendar weeks** of
   effort from a single contributor.
2. The `taffy` layout phase regresses frame budget by > 10 % (measured).
3. A class of visual artifacts we cannot solve in the layout model
   (e.g. real soft-wrap in the agent transcript) emerges.

If any trigger fires:

- **First fallback = `gpui-ce`**. It keeps the same GPU-native,
  Rust-only, sub-ms render promise, uses flexbox by default, and ports
  one-for-one from our custom painter. Cost: ~4 additional weeks, and
  coupling to an upstream with a small maintainer base. We would gate
  this behind a `shell = "gpui"` workspace feature so `editor-core`
  stays usable from either shell.
- **Second fallback = Tauri 2 for the chrome only, native wgpu canvas
  for the editor** (hybrid). This violates the literal text of PRD §2
  ("no embedded browser"); we would update the PRD *before* making the
  switch, with a measured input-latency report in hand showing we are
  still under 5 ms p95 on the editor surface.

We will **not** move to full Electron / Tauri shells where the editor
itself is a webview. That kills the entire reason this project exists.

## 10. Deliverables & schedule

| Week | Mission | Artifact |
|---|---|---|
| 1 | M29 | `editor-layout` crate + taffy wrap + `Measure` trait. Unit tests green. |
| 2 | M30.a | Port sidebar, tabstrip, breadcrumbs, status bar, title bar. |
| 3 | M30.b | Port agent panel, terminal header, command palette, settings overlay. `main.rs` shrinks. Snapshot tests. |
| 4 | M31 | Scissor rects, overflow modes, scrollbar migration. Long-string visual smoke. |
| 5 | M32 | Cursor-style parity pass, composer fix, popover model picker, reference-screenshot diff. Quality gate green on all three OSes. |

Each week produces a merge-ready branch named `mission/m29..m32-*`, a
`docs/missions/M29..M32_*.md` mission brief, and an updated
`MISSION_IMPLEMENTATION_STATUS.md`.

## 11. Why not the other things people will suggest

- **"Just use HTML via a WebView — it's easier."** It is, for chrome. But
  the editor itself is the 90 % case by frame time and we lose the input
  latency budget the moment we route keystrokes through a webview
  process. Cursor's own UX problems stem from exactly this.
- **"Use Flutter + Rust FFI."** Flutter's text engine is not built for
  monospace editor canvases with millions of visible glyph slots; we
  would end up reimplementing `glyphon` inside Skia. Also: Dart, on
  top of Rust, on top of `wgpu`, inside `winit`. No.
- **"Wait for Xilem."** It is research-grade. Betting the product on it
  would be irresponsible.
- **"Rewrite in Zed's crates."** See §3 option B — we keep that as an
  actual fallback with a concrete trigger, not a daydream.

## 12. Open questions

- Do we want `taffy`'s grid module (new in 0.5) for the editor
  canvas + minimap split? Probably yes; M31 will tell us.
- Do we expose a subset of `editor-layout` as a public crate for
  people writing extensions? Decide in M32.
- On Windows, `winit` 0.30 now supports a hit-test area for draggable
  chrome. Title-bar rework in M32 should adopt it to remove the
  system-painted title bar entirely.

---

*See also: `PRD.md`, `ARCHITECTURE.md`, `RENDERING_PIPELINE.md`,
`PERFORMANCE_BUDGETS.md`, `MISSION_IMPLEMENTATION_STATUS.md`.*
