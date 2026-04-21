//! Server-Sent Events: incremental `data:` extraction ([`SseBuffer`]) for provider streams.

use futures::stream::{BoxStream, Stream};
use futures::StreamExt;

use crate::error::ProviderError;

/// Yields each SSE `data` payload (skips `[DONE]`) from a `reqwest` byte stream.
pub(crate) fn iter_data_payloads<S>(
    byte_stream: S,
) -> BoxStream<'static, Result<String, ProviderError>>
where
    S: Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Send + 'static,
{
    Box::pin(async_stream::stream! {
        let mut byte_stream = byte_stream.boxed();
        let mut sse = SseBuffer::default();
        while let Some(item) = byte_stream.next().await {
            let chunk = match item {
                Ok(b) => b,
                Err(e) => {
                    yield Err(ProviderError::http(e));
                    return;
                }
            };
            let text = String::from_utf8_lossy(&chunk);
            for payload in sse.push_str(&text) {
                yield Ok(payload);
            }
        }
    })
}

/// Hold incomplete SSE event text until a blank line delimiter arrives.
#[derive(Default)]
pub struct SseBuffer {
    pending: String,
}

impl SseBuffer {
    /// Push a UTF-8 fragment; returns complete logical `data` payloads. OpenAI's `[DONE]` is omitted.
    pub fn push_str(&mut self, s: &str) -> Vec<String> {
        self.pending.push_str(&s.replace("\r\n", "\n"));
        let mut out = Vec::new();
        while let Some(idx) = self.pending.find("\n\n") {
            let block = self.pending[..idx].to_string();
            self.pending.drain(..idx + 2);
            if let Some(payload) = extract_data_payload(&block) {
                if payload != "[DONE]" {
                    out.push(payload);
                }
            }
        }
        out
    }
}

fn extract_data_payload(block: &str) -> Option<String> {
    let mut merged = Vec::new();
    for line in block.split('\n') {
        if line.is_empty() {
            continue;
        }
        if line.starts_with(':') {
            continue;
        }
        let Some(rest) = line.strip_prefix("data:") else {
            continue;
        };
        merged.push(rest.trim_start());
    }
    if merged.is_empty() {
        None
    } else {
        Some(merged.join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_back_to_back_events() {
        let mut b = SseBuffer::default();
        let out = b.push_str("data: {\"x\":1}\n\ndata: {\"y\":2}\n\n");
        assert_eq!(out.len(), 2);
        assert_eq!(out[0], "{\"x\":1}");
    }
}
