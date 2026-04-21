//! POST with retries on 429 / 5xx and optional `Retry-After` handling.

use std::time::Duration;

use backoff::backoff::Backoff;
use backoff::ExponentialBackoff;
use reqwest::header::{HeaderMap, RETRY_AFTER};
use reqwest::StatusCode;

use crate::error::ProviderError;

const DEFAULT_RETRIES: u32 = 3;

fn parse_retry_after(headers: &HeaderMap) -> Option<Duration> {
    let v = headers.get(RETRY_AFTER)?.to_str().ok()?;
    if let Ok(secs) = v.parse::<u64>() {
        return Some(Duration::from_secs(secs));
    }
    None
}

/// Sends a POST; on 429 or 5xx retries up to `DEFAULT_RETRIES` times.
pub async fn post_json_expect_success(
    client: &reqwest::Client,
    url: &str,
    headers: HeaderMap,
    body: serde_json::Value,
) -> Result<reqwest::Response, ProviderError> {
    let mut attempt: u32 = 0;
    let mut exp_backoff = ExponentialBackoff::default();
    loop {
        let resp = client
            .post(url)
            .headers(headers.clone())
            .json(&body)
            .send()
            .await
            .map_err(ProviderError::http)?;
        let status = resp.status();
        if status.is_success() {
            return Ok(resp);
        }
        let retry_after_hdr = parse_retry_after(resp.headers());
        let body_text = resp.text().await.unwrap_or_default();
        let retryable = status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error();
        if retryable && attempt < DEFAULT_RETRIES {
            let wait = retry_after_hdr.unwrap_or_else(|| {
                exp_backoff.next_backoff().unwrap_or_else(|| Duration::from_secs(1))
            });
            tracing::warn!(
                status = %status,
                attempt,
                wait_ms = wait.as_millis(),
                "retrying provider HTTP request"
            );
            tokio::time::sleep(wait).await;
            attempt += 1;
            continue;
        }
        return Err(ProviderError::http_status(status.as_u16(), body_text));
    }
}
