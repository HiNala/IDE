//! Maintainer-only smoke test: streams a short reply from configured providers.
//! Not run in CI (requires `ANTHROPIC_API_KEY` and/or a local Ollama).
//!
//! Run from repo root:
//! `cargo run -p editor-ai-provider --bin ai-smoke`

#![allow(clippy::print_stdout, clippy::print_stderr)]

use anyhow::Result;
use editor_ai_provider::config::load_or_create_default;
use editor_ai_provider::registry::ProviderRegistry;
use editor_ai_provider::secrets::SecretStore;
use editor_ai_provider::{ChatEvent, ChatRequest, ContentBlock, Message};
use futures::StreamExt;

#[tokio::main]
async fn main() -> Result<()> {
    let cfg = load_or_create_default(None)?;
    let secrets = SecretStore::new();
    let registry = ProviderRegistry::from_config(&cfg, &secrets)?;
    let Some(provider) = registry.active() else {
        eprintln!(
            "No active provider — check {} and API keys.",
            editor_ai_provider::config::providers_config_path().display()
        );
        return Ok(());
    };

    let model = cfg.active.model.clone();
    eprintln!("Using provider {} model {}", provider.id(), model);

    let mut stream = provider
        .chat(ChatRequest {
            model,
            system: Some("You are a terse assistant.".into()),
            messages: vec![Message::User {
                content: vec![ContentBlock::Text("Say the word hello and nothing else.".into())],
            }],
            tools: vec![],
            max_tokens: 64,
            temperature: Some(0.0),
            stop: vec![],
            stream: true,
        })
        .await?;

    while let Some(ev) = stream.next().await {
        match ev? {
            ChatEvent::TextDelta(s) => print!("{s}"),
            ChatEvent::ToolCall { name, .. } => eprintln!("\n[tool] {name}"),
            ChatEvent::Done { usage, .. } => {
                eprintln!(
                    "\n[done] input_tokens={} output_tokens={}",
                    usage.input_tokens, usage.output_tokens
                );
            }
            ChatEvent::Error(e) => {
                eprintln!("\n[error] {e}");
                break;
            }
        }
    }
    println!();
    Ok(())
}
