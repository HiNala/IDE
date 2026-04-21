//! Deterministic Anthropic SSE parsing (no network).

use editor_ai_provider::anthropic::AnthropicProvider;
use editor_ai_provider::provider::AiProvider;
use editor_ai_provider::rate_limit::MinuteRateLimit;
use editor_ai_provider::{ChatEvent, ChatRequest, ContentBlock, Message};
use futures::StreamExt;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn anthropic_stream_text_and_done() {
    let server = MockServer::start().await;
    let body = concat!(
        "event: message_start\n",
        "data: {\"type\":\"message_start\",\"message\":{\"role\":\"assistant\"}}\n\n",
        "event: content_block_delta\n",
        "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Hi\"}}\n\n",
        "event: message_delta\n",
        "data: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"input_tokens\":10,\"output_tokens\":2}}\n\n",
        "event: message_stop\n",
        "data: {\"type\":\"message_stop\"}\n\n",
    );
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream; charset=utf-8")
                .set_body_string(body),
        )
        .mount(&server)
        .await;

    let rate = MinuteRateLimit::new(120);
    let p = AnthropicProvider::new("sk-test", Some(server.uri().parse().expect("url")), rate)
        .expect("provider");

    let mut stream = p
        .chat(ChatRequest {
            model: "claude-test".into(),
            system: None,
            messages: vec![Message::User { content: vec![ContentBlock::Text("hello".into())] }],
            tools: vec![],
            max_tokens: 64,
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
                assert_eq!(usage.input_tokens, 10);
                assert_eq!(usage.output_tokens, 2);
                saw_done = true;
            }
            e => panic!("unexpected event: {e:?}"),
        }
    }
    assert!(saw_text);
    assert!(saw_done);
}
