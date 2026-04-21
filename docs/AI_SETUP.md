# Setting up API keys for AI providers

The IDE stores provider credentials in the **OS keychain** (Windows Credential Manager, macOS Keychain, or the freedesktop.org Secret Service on Linux). They are **never** written to `providers.toml` or other project files.

## Keychain entries

- **Service name:** `ide`
- **Account names:** `anthropic`, `openai`, optional `custom:<your-profile-name>` for custom OpenAI-compatible endpoints.

You can set secrets from your own small helper or a future `/settings` UI (M23+). Programmatically, use `editor_ai_provider::secrets::SecretStore` (`set_key` / `get_key` / `delete_key`).

## Environment fallback

If the keychain cannot be accessed (common on headless Linux CI or when no D-Bus secret service is running), the crate falls back to:

| Account | Environment variable |
|---------|----------------------|
| `anthropic` | `ANTHROPIC_API_KEY` |
| `openai` | `OPENAI_API_KEY` |
| `custom:my-server` | `IDE_CUSTOM_MY_SERVER_API_KEY` (non-alphanumeric characters in the profile name become `_`, then the suffix is uppercased) |

A warning is logged when fallback is used so you can migrate the value into the keychain on a workstation.

## Ollama (local)

No API key is required. Ensure Ollama is listening (default `http://localhost:11434`) and that `[providers.ollama]` in `providers.toml` matches your setup.

## Verifying

1. Copy or create `providers.toml` (see [`AI_PROVIDERS.md`](./AI_PROVIDERS.md)).
2. Store keys in the keychain or export the env vars above.
3. Run `cargo run -p editor-ai-provider --bin ai-smoke` and confirm you see streamed text and a `[done]` line with token counts.

Never paste API keys into logs, issues, or chat sessions.
