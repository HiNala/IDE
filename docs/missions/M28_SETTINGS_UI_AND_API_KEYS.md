# M28 — Settings UI & API Key Management

**Mission ID:** M28
**Prerequisites:** M20 complete (tools exist) and M27 complete (skills registered).
**Output:** A settings surface clean enough to stand up to scrutiny. A left-sidebar index of sections (AI Providers, Editor, Terminal, Skills, Keybindings-read-only, About). A right-pane detail view for the selected section. API keys stored in the OS keychain via the `keyring` integration from M19, entered through masked fields that round-trip to the keychain without ever being persisted to disk in plaintext. Per-provider enable/disable, default-model picker, custom base URL for OpenAI-compatible endpoints. Feature toggles for the major optional subsystems (metadata summarizer on/off, vector indexing on/off, skill enable/disable). Reset-to-default. No tabs, no wizards, no cute icons, no emoji. Opens on `Ctrl+,` (VS Code convention) as a full-window overlay that dismisses on `Esc`.
**Estimated scope:** 1-2 sessions.

---

## Read First

- `/00_MISSION_INDEX.md` — standing orders.
- `/00_V3_VISION.md` — Ring 4; must ship before the chat panel becomes practically usable.
- M19 — `SecretStore` + `ProviderRegistry` + config schema.
- M27 — `SkillRegistry` with enable/disable API surface.
- M10 — `PersistedState` (where feature toggles persist).
- Design reference: VS Code's settings pane (the spiritual benchmark — minimal, searchable, text-first). We do a simpler version.

---

## The Situation In Plain English

The AI chat panel we're about to ship is dead in the water without a way to set an API key, pick a model, and make a few sensible choices. M28 builds that surface. The temptation is to make it a beautiful complex thing with tabs, icons, animated transitions, and a dozen knobs. Resist. A settings panel is navigation furniture, not content; it should be invisible when not in use and unremarkable when in use. Users should enter, configure, and leave in under 30 seconds on a fresh install.

The design brief in one sentence: *a settings panel designed by someone who hates settings panels.* Text-first. Every control labeled in English. No secondary settings hidden behind "advanced" expanders (if a setting matters, show it; if it doesn't, delete it). Keyboard-navigable end to end — any user who never touches the mouse can still set every option. Dark by default, matching the rest of the editor; no separate theme for the settings view.

API keys are the one area where the UI has to earn its keep. The input must mask the value (`•••••••••`), never echo it to logs or the filesystem, round-trip to the OS keychain on save, and surface a clear "Key is set ✓" / "No key set" status. Users have to be able to *paste* keys in — a common failure mode of secret fields is blocking paste — so we explicitly allow it. Users also have to be able to verify a key works; a "Test connection" button pings `/v1/models` (or the provider equivalent) and reports success/failure in the same panel.

The seven sections are small enough that we can describe them here exhaustively.

**AI Providers.** The most important section. For each of the five providers (OpenAI, Anthropic, Gemini, Ollama, Custom): an enabled toggle, an API key field (not shown for Ollama which is keyless), a default-model dropdown, a custom base URL for OpenAI-compatible endpoints (hidden for non-custom providers). At the top: a global "Active provider" + "Active model" that the chat panel reads.

**Editor.** The handful of settings that genuinely matter: font size, line-ending default (auto / LF / CRLF), trim trailing whitespace on save (bool), ensure newline at EOF on save (bool), word-wrap (bool).

**Terminal.** Shell override path + shell arguments (defaults auto-detected but overridable), font size, scrollback lines (default 10000), default pane height percentage.

**Skills.** A list of every registered skill with an enabled checkbox and an "Open file" link that opens the SKILL.md in a buffer. Plus fields for additional skill directories (list with add/remove).

**Keybindings.** Read-only for V3. A searchable list of every shortcut and its command. Rebinding is V4+.

**About.** Version, commit hash, build date, quick links to the GitHub repo and docs.

Reset-to-defaults: per-section button. Confirmation banner on click; second press to confirm.

---

## Scope

**In scope:**
- `editor-settings` crate with the settings schema, loader, saver, validation.
- Settings UI — full-window overlay accessible via `Ctrl+,`.
- Six working sections as described above.
- Keyring-backed API key storage via M19's `SecretStore`.
- "Test connection" per provider.
- Import/export settings (JSON file; keys are NOT exported — they stay in keyring).
- Auto-save on change (no "Apply" button — the second you toggle a setting it takes effect and persists).
- Migration story for existing `state.json` — new fields default sensibly; old fields preserved.

**Out of scope:**
- A marketplace / plugin catalog (V4+).
- Themes / custom color schemes (V4+).
- Rebinding keyboard shortcuts (V4+).
- Per-language editor settings (V4+).
- Sync / cloud settings (V4+).
- Profile switching (e.g., "work" vs "personal") (V4+).

---

## North Star

Fresh install, first launch. `Ctrl+,`. Settings overlay fades in. Up-arrow lands on "AI Providers." Press Enter. Fields appear. Tab to API key, paste, Tab to Test connection, Enter. "Connection OK." Tab to Active provider, Enter, Down-arrow to OpenAI, Enter. Esc. Back to the editor. Open the chat panel. A working AI session. 20 seconds total.

---

## TODO List

### 1. Create `editor-settings` crate

- [ ] 1.1. `cargo new --lib crates/editor-settings`. Deps: `serde`, `serde_json`, `toml`, `thiserror`, `tracing`, `editor-core`, `editor-ai-provider` (for `SecretStore`), `editor-skills`.
- [ ] 1.2. Commit: `feat(settings): scaffold editor-settings crate`.

### 2. Settings schema

- [ ] 2.1. `src/schema.rs`:
  ```rust
  #[derive(Serialize, Deserialize, Clone, Debug, Default)]
  pub struct Settings {
      pub version: u32,                // schema version for migrations
      pub ai: AiSettings,
      pub editor: EditorSettings,
      pub terminal: TerminalSettings,
      pub skills: SkillsSettings,
      pub extra_skill_dirs: Vec<PathBuf>,
  }
  pub struct AiSettings {
      pub active_provider: Option<String>,
      pub active_model: Option<String>,
      pub providers: HashMap<String, ProviderConfig>,
      pub enabled_summarizer: bool,
      pub enabled_vector_index: bool,
      pub max_tokens_default: u32,
      pub temperature_default: Option<f32>,
  }
  pub struct ProviderConfig {
      pub enabled: bool,
      pub default_model: String,
      pub base_url: Option<String>,
      // api keys NOT serialized here — live in keyring
  }
  pub struct EditorSettings {
      pub font_size: f32,
      pub line_ending: LineEndingPreference,
      pub trim_trailing_whitespace_on_save: bool,
      pub ensure_newline_at_eof: bool,
      pub word_wrap: bool,
      pub undo_coalesce_ms: u64,
  }
  pub enum LineEndingPreference { Auto, Lf, Crlf }
  pub struct TerminalSettings {
      pub shell_override: Option<PathBuf>,
      pub shell_args: Vec<String>,
      pub font_size: f32,
      pub scrollback_lines: u32,
      pub default_height_pct: f32,
  }
  pub struct SkillsSettings {
      pub disabled: HashSet<String>,   // only disabled skills persisted
  }
  ```
- [ ] 2.2. Defaults: sensible for everyone (active_provider = None initially; editor.font_size = 14.0; terminal.scrollback_lines = 10000).
- [ ] 2.3. Commit: `feat(settings): schema with defaults`.

### 3. Loader / saver

- [ ] 3.1. File: `~/.config/ide/settings.toml` (platform-appropriate dirs via `directories` crate). Use TOML for human-readability.
- [ ] 3.2. `Settings::load() -> Settings` — parse if exists, else default + save. Handle migrations by `version` field.
- [ ] 3.3. `Settings::save(&self) -> Result<(), SettingsError>` — atomic write (temp + rename, reuse M06 pattern).
- [ ] 3.4. Auto-save: every mutation goes through a `SettingsStore` that debounces saves by 200 ms.
- [ ] 3.5. Commit: `feat(settings): load/save with atomic write`.

### 4. Import/export

- [ ] 4.1. `SettingsStore::export(path: &Path)` writes the current settings as formatted JSON, omitting anything secret.
- [ ] 4.2. `SettingsStore::import(path: &Path)` validates and applies a new settings file. Invalid fields are rejected with a clear error.
- [ ] 4.3. Keys stay in the keyring; they're never exported and the user re-enters them on a new machine. Document this in `/docs/SETTINGS.md`.
- [ ] 4.4. Commit: `feat(settings): import/export JSON`.

### 5. SettingsPanel UI

- [ ] 5.1. `editor-ui::SettingsPanel`:
  ```rust
  pub struct SettingsPanel {
      pub visible: bool,
      pub active_section: Section,
      pub scroll: f32,
      pub draft: Settings,                      // current in-memory state
      pub section_states: HashMap<Section, SectionState>,
  }
  pub enum Section {
      AiProviders,
      Editor,
      Terminal,
      Skills,
      Keybindings,
      About,
  }
  ```
- [ ] 5.2. Layout: left column 220px wide, lists the six sections; right column fills the rest, renders the active section's controls.
- [ ] 5.3. Rendering: every control is text + a quad background. Text fields: a single-row bordered box; toggles: `[ ]` / `[x]`; dropdowns: the current value with a `▾` that opens a small list below. No icons, no image assets.
- [ ] 5.4. Font: same Geist / JetBrains Mono as the rest. Body size 14pt, section headers 16pt semibold.
- [ ] 5.5. Background: same zinc as the editor (`#18181B`). Section-selected row: a thin accent-blue left bar (2px) + slightly lighter background.
- [ ] 5.6. Commit: `feat(ui): SettingsPanel shell rendering`.

### 6. Section: AI Providers

- [ ] 6.1. Top: Active Provider + Active Model dropdowns (linked — changing provider resets model to that provider's default).
- [ ] 6.2. Below: one card per provider.
  ```
  ┌─ OpenAI ─────────────────────────────────────┐
  │ [x] Enabled                                   │
  │ API Key   ••••••••••••••••  [Change] [Test]   │
  │ Default   gpt-5          ▾                    │
  └───────────────────────────────────────────────┘
  ```
- [ ] 6.3. `[Change]` swaps in a masked text field; on Enter saves to keyring via `SecretStore::set_key(provider, value)`; shows `✓ Saved`.
- [ ] 6.4. `[Test]` calls provider.chat with a trivial prompt ("hi"); on success shows `✓ Connection OK <latency>ms`; on failure shows `✗ <error>`.
- [ ] 6.5. For Ollama: hide API key; show base URL (default `http://localhost:11434`); `[Test]` pings `/api/tags`.
- [ ] 6.6. For Custom: show base URL field required; key field present; model entry is a free-text field (since we can't enumerate remote model lists in general).
- [ ] 6.7. Below provider cards: "Advanced" section with `enabled_summarizer`, `enabled_vector_index`, `max_tokens_default`, `temperature_default`.
- [ ] 6.8. Commit: `feat(settings): AI Providers section`.

### 7. Section: Editor

- [ ] 7.1. Five controls per the schema. Font size is a numeric spinner 8.0-32.0.
- [ ] 7.2. Changing font size takes effect on next redraw — immediate feedback.
- [ ] 7.3. Commit: `feat(settings): Editor section`.

### 8. Section: Terminal

- [ ] 8.1. Fields per schema. Detected shell shown as placeholder below the override field: "Detected: /bin/zsh — leave blank to use detection."
- [ ] 8.2. Commit: `feat(settings): Terminal section`.

### 9. Section: Skills

- [ ] 9.1. List every skill from `SkillRegistry`:
  ```
  [x] using-terminal       (built-in)
  [x] using-git            (built-in)
  [x] writing-rust         (built-in)
  [x] my-team-style        (workspace) [Open]
  ```
- [ ] 9.2. Checkbox toggles `SkillsSettings.disabled`. Registry reflects it on next reload.
- [ ] 9.3. "[Open]" opens the SKILL.md as a buffer so the user can edit.
- [ ] 9.4. Below the list: "Additional skill directories" — a managed list of paths with `[+ Add]` and `[x]` per entry.
- [ ] 9.5. Commit: `feat(settings): Skills section`.

### 10. Section: Keybindings (read-only)

- [ ] 10.1. Static list, searchable via a filter input at the top. Columns: command, keybinding, (for V4+) scope.
- [ ] 10.2. Pulled from a central keymap table — which should already exist in `editor-input` from M05; if not, create one in this mission.
- [ ] 10.3. Commit: `feat(settings): Keybindings section (read-only)`.

### 11. Section: About

- [ ] 11.1. Static info: app name, version (from `CARGO_PKG_VERSION`), commit hash (via `git-version` build script or env at build time), build date. Three links: repo, docs, changelog.
- [ ] 11.2. Commit: `feat(settings): About section`.

### 12. Input and focus flow

- [ ] 12.1. `Ctrl+,` shows the panel. `Esc` hides. Clicking outside the inner panel (if we use a dim backdrop) also hides — actually, to avoid a "lost my unsaved changes" feel with auto-save, there's no such trap; Esc only.
- [ ] 12.2. Tab / Shift-Tab cycle through focusable elements. Arrow keys move within a section list. Enter activates.
- [ ] 12.3. Search: the Keybindings and Skills sections both have `/` to focus their filter input.
- [ ] 12.4. Commit: `feat(input): SettingsPanel focus and keyboard nav`.

### 13. Reset-to-defaults

- [ ] 13.1. Per-section `[Reset to defaults]` button at the bottom.
- [ ] 13.2. First press shows a warning banner: "This will clear {section}'s settings. Press again to confirm." Second press within 5 seconds resets.
- [ ] 13.3. Reset does NOT touch keyring secrets (that's a separate action in AI Providers → "Forget key").
- [ ] 13.4. Commit: `feat(settings): per-section reset with confirmation`.

### 14. Migration

- [ ] 14.1. If an existing `state.json` has relevant fields (font_size, window state, etc.), migrate into the new settings schema on first load. Preserve `state.json` contents but stop writing to them.
- [ ] 14.2. Commit: `feat(settings): migrate legacy state.json fields`.

### 15. Styling pass — the polish

- [ ] 15.1. Spacing: consistent 16px between controls, 24px between sections, 32px panel padding.
- [ ] 15.2. Every interactive element has a visible focus ring in accent blue, 2px, 4px offset.
- [ ] 15.3. Disabled controls: 50% opacity, no cursor change.
- [ ] 15.4. Hover state on buttons: slightly lighter background; 50 ms transition (not longer — performance).
- [ ] 15.5. Masked API key field renders bullets as `•`, not `*`. Small detail, better visual.
- [ ] 15.6. Commit: `polish(settings): spacing, focus, hover states`.

### 16. Performance budget

- [ ] 16.1. Opening the settings panel must not drop a frame. All sections' controls are pre-computed on open; switching sections is a cheap view swap.
- [ ] 16.2. A change to any setting propagates via the usual frame loop — no synchronous disk writes.
- [ ] 16.3. Benchmark: render cost of settings panel at frame time: < 1 ms extra.
- [ ] 16.4. Commit: `perf(settings): ensure zero hitches on open/section-swap`.

### 17. Quality gates + docs

- [ ] 17.1. Standard gates.
- [ ] 17.2. `/docs/SETTINGS.md` for users: every setting, what it does, where the file lives, how keys are stored.
- [ ] 17.3. Manual: walk through the North Star scenario. 20-30 seconds from open to first chat prompt.
- [ ] 17.4. Tag: `git tag -a m28-complete -m "M28 complete: settings + API key management"`. Push.

---

## Validation / Acceptance Criteria

1. Quality gates pass.
2. `Ctrl+,` opens the panel in under 16 ms (one frame).
3. Every control in every section works.
4. Test Connection succeeds against a real OpenAI key.
5. Exported settings JSON is human-readable; re-imported it reproduces the same state.
6. Keys round-trip through keyring and never appear on disk.
7. `m28-complete` tag pushed.

## Testing Requirements

- Unit: schema default + round-trip + migration from a fixture `state.json`.
- Unit: `SecretStore` round-trip for each provider.
- Integration: set a key via UI (simulated), observe keyring get/set; set active provider, verify chat panel reads the new value.
- Manual: full walk-through on each OS.

## Git Commit Strategy

13-15 commits. Push after items 3, 6, 9, 12, 15, 17.

## Handoff to M21

M21's summarizer reads `AiSettings.enabled_summarizer` and its provider/model selection from settings. Plumb at M21 time.

## Handoff to M22

Same for vector indexing and embedder selection.

## Handoff to M23

The chat panel reads `AiSettings.active_provider` + `active_model`. When the user changes them in settings, the chat panel reflects immediately on next turn.

---

## Standing Orders Reminder

- Settings is navigation, not content. Minimal, labeled, invisible when not in use.
- Never echo, log, or persist an API key in plaintext. Keyring only.
- Test Connection is load-bearing. If it doesn't work, users won't trust the panel.
- Auto-save: never a "Save" button. Every mutation is immediate.
- If you're tempted to add a new knob that wasn't in the plan, ask: does removing it hurt? If no, don't add it.

Go.
