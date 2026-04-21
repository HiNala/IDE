//! Ollama path uses OpenAI-compatible `/v1/chat/completions` (same SSE as OpenAI).

use editor_ai_provider::ollama::OllamaProvider;
use editor_ai_provider::provider::AiProvider;
use editor_ai_provider::rate_limit::MinuteRateLimit;
use editor_ai_provider::{ChatEvent, ChatRequest, ContentBlock, Message};
use futures::StreamExt;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn ollama_stream_emits_text_then_done() {
    let server = MockServer::start().await;
    let body = concat!(
        "data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"Hi\"}}]}\n\n",
        "data: {\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}],",
        "\"usage\":{\"prompt_tokens\":1,\"completion_tokens\":1}}\n\n",
        "data: [DONE]\n\n",
    );
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream; charset=utf-8")
                .set_body_string(body),
        )
        .mount(&server)
        .await;

    let rate = MinuteRateLimit::new(120);
    let p = OllamaProvider::new(Some(server.uri().parse().expect("url")), rate).expect("provider");

    let mut stream = p
        .chat(ChatRequest {
            model: "llama3.2".into(),
            system: None,
            messages: vec![Message::User { content: vec![ContentBlock::Text("hi".into())] }],
            tools: vec![],
            max_tokens: 32,
            temperature: None,
            stop: vec![],
            stream: true,
        })
        .await
        .expect("chat");

    let mut saw_text = false;
    let mut saw_done = false;
    while let Some(ev) = stream.next().await {
        match ev.expect("event") {
            ChatEvent::TextDelta(s) => {
                assert_eq!(s, "Hi");
                saw_text = true;
            }
            ChatEvent::Done { usage, .. } => {
                assert_eq!(usage.input_tokens, 1);
                assert_eq!(usage.output_tokens, 1);
                saw_done = true;
            }
            e => panic!("unexpected event: {e:?}"),
        }
    }
    assert!(saw_text);
    assert!(saw_done);
}
