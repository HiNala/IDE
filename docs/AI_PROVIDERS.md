# AI providers (M19)

The `editor-ai-provider` crate exposes a single `AiProvider` trait (`crates/editor-ai-provider`). Application code (for example the future M23 chat panel) works against `ChatRequest` / `ChatEvent` streams and does not branch on vendor.

## Implemented backends

| Kind | `AiProvider::id()` | Wire format |
|------|---------------------|-------------|
| Anthropic | `anthropic` | Messages API, SSE (`/v1/messages`) |
| OpenAI | `openai` | Chat Completions, SSE (`/v1/chat/completions`) |
| Ollama | `ollama` | OpenAI-compatible path on the Ollama base URL (`/v1/chat/completions`); model list via `/api/tags` |
| Custom HTTP | `custom:<name>` | Same as OpenAI; base URL points at your vLLM / LM Studio / TGI server |

Gemini is intentionally deferred (see M19 mission notes).

## Streaming contract

- **Input:** `ChatRequest` with `stream: true` (required for the current implementations).
- **Output:** `futures::Stream<Item = Result<ChatEvent, ProviderError>>`.
- **Completion:** Every successful stream ends with `ChatEvent::Done` carrying `Usage` (input/output token counts when the provider reports them).
- **Tools:** `ToolDef` in the request; `ChatEvent::ToolCall` in the stream. Tool execution and follow-up `Message::ToolResult` turns are M20/M23.

## Configuration

File path from `providers_config_path()` in `editor-ai-provider::config`:

- Linux: `~/.config/ide/providers.toml`
- Windows: `%APPDATA%\ide\providers.toml`
- macOS: `~/Library/Application Support/ide/providers.toml`

On first read, a default file is created if missing. Active provider and model live under `[active]`. Per-provider tables live under `[providers.*]`.

## Credentials

API keys are **not** stored in the TOML file. They are read from the OS keychain via the `keyring` crate (`service` `ide`, per-provider account name), with environment-variable fallback when the secret service is unavailable. Details: [`AI_SETUP.md`](./AI_SETUP.md).

## Rate limits and retries

- **Rate limit:** A simple per-provider sliding window limits chat calls per rolling minute (`[limits].rate_limit_per_minute` in `providers.toml`).
- **Retries:** HTTP POST uses the `backoff` crate’s exponential policy (with jitter) on HTTP 429 and 5xx (up to three attempts), honoring `Retry-After` when present (`crates/editor-ai-provider/src/http.rs`).
- **SSE:** Streaming bodies are parsed incrementally (`SseBuffer` in `sse.rs`); malformed JSON lines are skipped with a warning where possible.

## Manual smoke test

```bash
cargo run -p editor-ai-provider --bin ai-smoke
```

Requires a valid `providers.toml`, keys in the keychain or env, and (for Anthropic) network access. Not run in CI.

## Tests

Integration tests use `wiremock` for deterministic SSE fixtures under `crates/editor-ai-provider/tests/`.
