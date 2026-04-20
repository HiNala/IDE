# M19 — AI Provider Abstraction

**Mission ID:** M19
**Prerequisites:** M18 complete. The developer-facing half of V3 is stable.
**Output:** A new `editor-ai-provider` crate providing a uniform `AiProvider` trait over Anthropic, OpenAI, Gemini, Ollama (local), and arbitrary OpenAI-compatible HTTP endpoints. Streaming responses via an async event stream. Support for tool-use / function calling in the request/response cycle. API keys stored in the OS keychain via the `keyring` crate. Rate limiting, retries, token accounting. Configuration via `~/.config/ide/providers.toml`. **No UI** — M19 is pure plumbing; M23 builds the chat panel that uses it.
**Estimated scope:** 2-3 sessions.

---

## Read First

- `/00_MISSION_INDEX.md` — standing orders.
- `/00_V3_VISION.md` — Ring 3 agent substrate; this is the first AI-facing mission.
- `https://platform.claude.com/docs/en/build-with-claude/streaming` — Anthropic Messages API streaming.
- `https://platform.claude.com/docs/en/agents-and-tools/tool-use/overview` — Anthropic tool-use semantics.
- `https://platform.openai.com/docs/api-reference/chat` — OpenAI chat completions with function calling.
- `https://ai.google.dev/api/generate-content` — Gemini API.
- `https://github.com/ollama/ollama/blob/main/docs/api.md` — Ollama's OpenAI-compatible and native APIs.
- `https://docs.rs/keyring/` — OS-keychain credential storage (Windows Credential Manager, macOS Keychain, Linux Secret Service).
- `https://docs.rs/reqwest/` — async HTTP client.
- `https://docs.rs/eventsource-stream/` — Server-Sent Events parsing.

---

## The Situation In Plain English

By the end of 2026, most code will be written by LLM API calls. Any editor that wants to be serious about agent use needs a clean, well-designed provider layer. Most of today's editors get this half-right — Cursor and Windsurf each support multiple providers, but they're opinionated toward their preferred model and the provider abstraction leaks through in subtle ways. We do it right from the start: one trait, many implementations, zero per-provider special cases in the consuming code.

The tricky part is that "chat with tool use" is the *only* interaction model we care about for V3. We don't do autocomplete-as-you-type (that's V4+ and needs a different latency profile anyway). So the trait surface shrinks to essentially: "take a request (system prompt + conversation + available tools), return an async stream of events (text deltas, tool-call events, done, error)." Clean and small.

The five initial providers:

1. **Anthropic** (Claude, primary target). Uses the Messages API with server-sent events. Tool-use expressed as `tool_use` / `tool_result` content blocks. The most recent model, as of the knowledge cutoff, is `claude-opus-4-7`.
2. **OpenAI** (GPT series). Chat Completions API with `tool_calls` on delta messages. Stream format is SSE with `data: {...}` lines.
3. **Gemini** (Google). Similar shape but a different response envelope. Less critical than Anthropic and OpenAI for V3; ship if time permits, stub with feature flag otherwise.
4. **Ollama** (local). Exposes an OpenAI-compatible endpoint at `http://localhost:11434/v1`. In practice, for local use, we default to the native Ollama API at `/api/chat` which is very close to OpenAI's.
5. **Custom HTTP** (arbitrary OpenAI-compatible endpoint). User-configurable base URL + API key. Covers Hugging Face TGI, vLLM, LM Studio, and anyone else shipping an OpenAI-compatible surface.

All five live behind the same trait. The calling code (M23's chat panel, M20's tool loop) never knows which provider is active.

Credentials are the other half. We never store API keys in plaintext config files on disk. We use the `keyring` crate, which wraps Windows Credential Manager, macOS Keychain, and Linux Secret Service. Users configure keys via a `/settings` command or — more commonly in V3 — via environment variables on first launch, which get offered for migration into the keychain.

---

## Scope

**In scope:**
- `editor-ai-provider` crate with the `AiProvider` trait.
- Implementations: `AnthropicProvider`, `OpenAIProvider`, `OllamaProvider`, `CustomHttpProvider`.
- Streaming via async `futures::Stream<Item = ChatEvent>`.
- Tool-use support in request (`Tool` definitions) and response (`ChatEvent::ToolCall`).
- API key storage via `keyring`.
- Configuration via `~/.config/ide/providers.toml`.
- Rate limiting via a simple per-provider token bucket.
- Exponential-backoff retry with jitter on 429 / 5xx.
- Token accounting (input + output tokens reported on stream end).

**Out of scope:**
- GeminiProvider (nice-to-have; defer to M23 if time pressure arrives). Add as follow-up.
- Local model hosting beyond Ollama (llama.cpp server support via OpenAI-compatible mode is covered by `CustomHttpProvider`).
- Vision / multimodal inputs (V4+; V3 is text-only agent interactions).
- Any UI (M23).
- Cost estimation / budget controls beyond raw token counts (V4+).

---

## North Star

A consumer writes:

```rust
let provider = ProviderRegistry::active();   // whatever the user has configured
let mut stream = provider.chat(ChatRequest {
    model: "claude-opus-4-7".into(),
    system: Some("You are a coding assistant.".into()),
    messages: vec![Message::user("Read /src/main.rs and tell me what it does.")],
    tools: vec![read_file_tool_def()],
    max_tokens: 4096,
    stream: true,
}).await?;

while let Some(event) = stream.next().await {
    match event? {
        ChatEvent::TextDelta(s) => print!("{s}"),
        ChatEvent::ToolCall(call) => { /* route through M20 */ },
        ChatEvent::ToolResult(_) => {},
        ChatEvent::Done { usage } => { /* record tokens */ break; },
        ChatEvent::Error(e) => return Err(e.into()),
    }
}
```

And the same code works against Claude, GPT, or a local Ollama model, with zero provider-specific branches.

---

## TODO List

### 1. Create `editor-ai-provider` crate

- [ ] 1.1. `cargo new --lib crates/editor-ai-provider`. Deps: `reqwest` (with `json`, `stream`, `rustls-tls` features), `tokio`, `futures`, `serde` + `serde_json`, `eventsource-stream`, `keyring`, `toml`, `thiserror`, `anyhow`, `tracing`, `backoff`, `url`.
- [ ] 1.2. Commit: `feat(ai-provider): scaffold editor-ai-provider crate`.

### 2. Core trait and types

- [ ] 2.1. `src/types.rs`:
  ```rust
  pub struct ChatRequest {
      pub model: String,
      pub system: Option<String>,
      pub messages: Vec<Message>,
      pub tools: Vec<ToolDef>,
      pub max_tokens: u32,
      pub temperature: Option<f32>,
      pub stop: Vec<String>,
      pub stream: bool,
  }
  pub enum Message {
      User { content: Vec<ContentBlock> },
      Assistant { content: Vec<ContentBlock> },
      ToolResult { tool_call_id: String, content: String, is_error: bool },
  }
  pub enum ContentBlock {
      Text(String),
      ToolUse { id: String, name: String, input: serde_json::Value },
  }
  pub struct ToolDef {
      pub name: String,
      pub description: String,
      pub input_schema: serde_json::Value,   // JSON Schema
  }
  pub enum ChatEvent {
      TextDelta(String),
      ToolCall { id: String, name: String, input: serde_json::Value },
      Done { usage: Usage, stop_reason: StopReason },
      Error(ProviderError),
  }
  pub struct Usage {
      pub input_tokens: u32,
      pub output_tokens: u32,
  }
  pub enum StopReason { EndTurn, MaxTokens, StopSequence, ToolUse, Other(String) }
  ```
- [ ] 2.2. `src/provider.rs`:
  ```rust
  #[async_trait::async_trait]
  pub trait AiProvider: Send + Sync {
      fn id(&self) -> &str;                   // "anthropic", "openai", "ollama", etc.
      fn available_models(&self) -> Vec<ModelInfo>;
      async fn chat(&self, req: ChatRequest)
          -> Result<Pin<Box<dyn Stream<Item = Result<ChatEvent, ProviderError>> + Send>>, ProviderError>;
  }
  ```
- [ ] 2.3. Commit: `feat(ai-provider): core trait and request/event types`.

### 3. `AnthropicProvider`

- [ ] 3.1. `src/anthropic.rs`. Struct holds `reqwest::Client`, `api_key`, `base_url` (default `https://api.anthropic.com`).
- [ ] 3.2. Request transform: map our `ChatRequest` to Anthropic's `/v1/messages` body. Notably: Anthropic takes `system` as a top-level string (we already have it); `messages` takes alternating user/assistant with content as an array of blocks; `tools` maps directly.
- [ ] 3.3. Response transform (streaming): parse the SSE stream via `eventsource-stream`. Anthropic events include `message_start`, `content_block_start`, `content_block_delta`, `content_block_stop`, `message_delta`, `message_stop`. Map to our `ChatEvent`s:
  - `content_block_delta` with `text_delta` → `TextDelta`.
  - `content_block_start` of type `tool_use` + subsequent `input_json_delta`s that we accumulate → on `content_block_stop`, emit `ToolCall`.
  - `message_delta` with `stop_reason` → keep until `message_stop` → emit `Done { usage, stop_reason }`.
- [ ] 3.4. Headers: `x-api-key`, `anthropic-version: 2023-06-01`, `content-type: application/json`.
- [ ] 3.5. Tool-use semantics: when Anthropic returns `stop_reason: "tool_use"`, the caller is expected to execute tools and send results back via a follow-up request with `ToolResult` messages. That orchestration lives in M20/M23, not here; this mission just faithfully surfaces the events.
- [ ] 3.6. Unit tests: mock HTTP server returning sample SSE streams; assert correct event mapping.
- [ ] 3.7. Commit: `feat(ai-provider): Anthropic provider with streaming + tool use`.

### 4. `OpenAIProvider`

- [ ] 4.1. `src/openai.rs`. Similar shape, different endpoint (`/v1/chat/completions`).
- [ ] 4.2. Request transform: OpenAI's messages are `{role, content}` (content may be a string or an array of parts); tools are `tools: [{type: "function", function: {...}}]`.
- [ ] 4.3. Response transform (streaming): OpenAI's SSE emits `data: {choices: [{delta: {...}}]}` per chunk. Accumulate `delta.content` into `TextDelta`s. Tool calls are in `delta.tool_calls`, progressively filled — emit `ToolCall` once `finish_reason == "tool_calls"`.
- [ ] 4.4. Headers: `Authorization: Bearer <key>`.
- [ ] 4.5. Commit: `feat(ai-provider): OpenAI provider`.

### 5. `OllamaProvider`

- [ ] 5.1. `src/ollama.rs`. Default `base_url: http://localhost:11434`.
- [ ] 5.2. Use Ollama's OpenAI-compatible endpoint at `/v1/chat/completions` — this lets us reuse most of the OpenAI transform code with just a base-URL swap. Tool-use is supported in Ollama for compatible local models (e.g., Llama 3.1 Instruct, Qwen 2.5, etc.).
- [ ] 5.3. No API key required (it's local). Provider surface accepts `Option<String>` for the key.
- [ ] 5.4. `available_models()` can query `GET /api/tags` to enumerate installed local models.
- [ ] 5.5. Commit: `feat(ai-provider): Ollama provider for local models`.

### 6. `CustomHttpProvider`

- [ ] 6.1. `src/custom.rs`. Configurable base URL, API key, and model list. Uses the OpenAI-compatible request/response format.
- [ ] 6.2. Covers Hugging Face TGI, vLLM, LM Studio, any OpenAI-compatible local inference server.
- [ ] 6.3. Commit: `feat(ai-provider): custom OpenAI-compatible provider`.

### 7. Secret storage via `keyring`

- [ ] 7.1. `src/secrets.rs`:
  ```rust
  pub struct SecretStore { service_name: String }
  impl SecretStore {
      pub fn new() -> Self { Self { service_name: "ide".into() } }
      pub fn set_key(&self, provider: &str, value: &str) -> Result<(), KeyringError>;
      pub fn get_key(&self, provider: &str) -> Result<Option<String>, KeyringError>;
      pub fn delete_key(&self, provider: &str) -> Result<(), KeyringError>;
  }
  ```
- [ ] 7.2. Each provider's key lives under `keyring::Entry::new("ide", "anthropic")` etc.
- [ ] 7.3. On Linux, the Secret Service requires a running DBus + secret service daemon (GNOME Keyring, KDE KWallet, KeePassXC with Secret Service). If unavailable, fall back to reading from the environment variable `ANTHROPIC_API_KEY` / `OPENAI_API_KEY` / etc., with a log warning. Document in `/docs/AI_SETUP.md`.
- [ ] 7.4. Commit: `feat(ai-provider): OS-keychain credential storage`.

### 8. Configuration file

- [ ] 8.1. `~/.config/ide/providers.toml` (Windows: `%APPDATA%\ide\providers.toml`, macOS: `~/Library/Application Support/ide/providers.toml`):
  ```toml
  [active]
  provider = "anthropic"
  model = "claude-opus-4-7"

  [providers.anthropic]
  enabled = true
  # api key stored in OS keychain

  [providers.openai]
  enabled = true

  [providers.ollama]
  enabled = true
  base_url = "http://localhost:11434"

  [providers.custom.my-vllm]
  enabled = false
  base_url = "https://my-inference-server.example.com/v1"
  models = ["my-custom-model"]

  [limits]
  max_tokens_default = 4096
  rate_limit_per_minute = 60
  ```
- [ ] 8.2. Parse on startup; create default if missing.
- [ ] 8.3. Commit: `feat(ai-provider): TOML configuration file`.

### 9. Provider registry

- [ ] 9.1. `src/registry.rs`:
  ```rust
  pub struct ProviderRegistry {
      providers: HashMap<String, Arc<dyn AiProvider>>,
      active: Option<String>,
  }
  impl ProviderRegistry {
      pub fn from_config(config: &ProviderConfig, secrets: &SecretStore) -> Self;
      pub fn active(&self) -> Option<Arc<dyn AiProvider>>;
      pub fn set_active(&mut self, id: &str) -> Result<(), RegistryError>;
      pub fn list(&self) -> Vec<&str>;
  }
  ```
- [ ] 9.2. Commit: `feat(ai-provider): provider registry`.

### 10. Rate limiting and retries

- [ ] 10.1. Token-bucket limiter per provider. Use `governor` crate or a simple Mutex-guarded counter.
- [ ] 10.2. On 429 (rate limited) or 5xx: retry with exponential backoff + jitter using the `backoff` crate. Max 3 retries. Respect `Retry-After` header when present.
- [ ] 10.3. Commit: `feat(ai-provider): rate limiting and retry with backoff`.

### 11. Token accounting

- [ ] 11.1. Every completed `chat` stream ends with `ChatEvent::Done { usage }`.
- [ ] 11.2. `TokenLedger` struct collects usage across a session; used by M23's chat panel to display cumulative tokens + estimated cost.
- [ ] 11.3. Commit: `feat(ai-provider): token ledger`.

### 12. Integration testing harness

- [ ] 12.1. Mock HTTP server using `wiremock` for deterministic tests. Sample SSE responses for each provider, verify event mapping.
- [ ] 12.2. Live-test script `scripts/ai-smoke.rs` that hits each configured provider with a simple "say hello" and prints the result. Not run in CI (depends on real keys); a maintainer runs it manually before release.
- [ ] 12.3. Commit: `test(ai-provider): wiremock integration tests and smoke script`.

### 13. Quality gates + documentation

- [ ] 13.1. Standard gates.
- [ ] 13.2. Write `/docs/AI_PROVIDERS.md` and `/docs/AI_SETUP.md`. The latter is user-facing: "How to set up your API key."
- [ ] 13.3. Tag: `git tag -a m19-complete -m "M19 complete: AI provider abstraction"`; push.

---

## Validation / Acceptance Criteria

1. Quality gates pass.
2. All four providers compile and pass unit tests against mocked responses.
3. Live smoke test with a real Anthropic key returns a streaming response.
4. Live smoke test with a local Ollama instance returns a streaming response.
5. API keys round-trip through the OS keychain.
6. No UI yet; that's M23.
7. `m19-complete` tag pushed.

## Testing Requirements

- Unit: mocked SSE for each provider.
- Integration: live smoke against real Anthropic + Ollama (manual).
- Keyring round-trip test on each OS (in CI via `keyring` test features or manual).

## Git Commit Strategy

10-13 commits. Push after items 3, 5, 7, 10, 13.

## Handoff to M20

M20 is the tool-use API — what the LLM can *do* via the provider we just built. M19 produces `ChatEvent::ToolCall`; M20 defines what tools exist, how they execute, and how the results flow back.

---

## Standing Orders Reminder

- Never log API keys. Never. Not even at trace level.
- Never put credentials in config files. Always `keyring`.
- Every provider response path must be resilient to malformed JSON — foundation model providers occasionally send garbled SSE frames.
- Tool-use JSON schemas MUST use `strict: true` where the provider supports it. Drift in schema interpretation is the source of most agent-tool bugs.

Go.
