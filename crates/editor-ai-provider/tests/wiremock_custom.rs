//! Custom OpenAI-compatible base URL (`custom:<name>` in registry).

use editor_ai_provider::custom::CustomHttpProvider;
use editor_ai_provider::provider::AiProvider;
use editor_ai_provider::rate_limit::MinuteRateLimit;
use editor_ai_provider::{ChatEvent, ChatRequest, ContentBlock, Message};
use futures::StreamExt;
use url::Url;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn custom_openai_compatible_stream() {
    let server = MockServer::start().await;
    let body = concat!(
        "data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"OK\"}}]}\n\n",
        "data: {\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}],",
        "\"usage\":{\"prompt_tokens\":2,\"completion_tokens\":1}}\n\n",
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
    let base: Url = format!("{}/v1", server.uri().trim_end_matches('/')).parse().expect("base url");
    let p = CustomHttpProvider::new("test-srv", base, None, vec!["my-model".into()], rate)
        .expect("provider");

    let mut stream = p
        .chat(ChatRequest {
            model: "my-model".into(),
            system: None,
            messages: vec![Message::User { content: vec![ContentBlock::Text("x".into())] }],
            tools: vec![],
            max_tokens: 16,
            temperature: None,
            stop: vec![],
            stream: true,
        })
        .await
        .expect("chat");

    let mut saw = false;
    while let Some(ev) = stream.next().await {
        match ev.expect("event") {
            ChatEvent::TextDelta(s) => {
                assert_eq!(s, "OK");
                saw = true;
            }
            ChatEvent::Done { .. } => {}
            e => panic!("unexpected event: {e:?}"),
        }
    }
    assert!(saw);
    assert_eq!(p.id(), "custom:test-srv");
    assert_eq!(p.registry_key(), "test-srv");
}
