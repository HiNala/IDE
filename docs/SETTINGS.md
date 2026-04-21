# Settings (M28)

## Where settings live

- **Primary file:** `%APPDATA%` (Windows) / `~/.config` (Linux) / Application Support (macOS) under `ide/settings.toml` (see [`editor_settings::paths::settings_file_path`](crates/editor-settings/src/paths.rs)).
- **Legacy session file:** `state.json` in the same app config folder still stores window geometry, last file, and (for compatibility) skill disable lists; the editor syncs skill toggles into both `settings.toml` and `state.json` on save.

## API keys

- Keys are stored **only** in the OS keychain via the `keyring` crate (service name `ide`, account per provider, e.g. `openai`, `anthropic`, `gemini`, `custom:…`). They are **not** written to `settings.toml` or exported JSON.
- Environment fallbacks (when no keychain entry exists) follow `SecretStore` in [`editor-ai-provider/src/secrets.rs`](crates/editor-ai-provider/src/secrets.rs) (e.g. `OPENAI_API_KEY`).

## Import / export

- **Export** writes a JSON file with the same structure as settings, **without** any secret material (see `SettingsExport` in `editor-settings`).
- **Import** validates ranges and replaces in-memory settings, then saves TOML. Re-enter API keys on a new machine after import.

## UI

- **Ctrl+,** (Primary+,): open or close the full-window overlay.
- **Esc:** closes the overlay (or cancels API key entry). When the overlay is closed, **Esc** still maps to the app quit binding from `editor-input` (same as before M28).

## Integration points for later missions

- **M21 / M22:** `AiSettings.enabled_summarizer`, `enabled_vector_index`.
- **M23:** `AiSettings.active_provider` and `active_model` for the chat panel.
