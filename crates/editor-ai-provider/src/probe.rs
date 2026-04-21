//! Blocking HTTP checks for "Test connection" in settings (M28).

use std::time::{Duration, Instant};

/// `GET {base}/v1/models` — OpenAI and OpenAI-compatible servers.
pub fn probe_openai_compatible(base_url: &str, api_key: Option<&str>) -> Result<u128, String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|e: reqwest::Error| e.to_string())?;
    let base = base_url.trim_end_matches('/');
    let url = format!("{base}/v1/models");
    let start = Instant::now();
    let mut req = client.get(&url);
    if let Some(k) = api_key.filter(|s| !s.is_empty()) {
        req = req.bearer_auth(k);
    }
    let resp = req.send().map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }
    Ok(start.elapsed().as_millis())
}

/// `GET {base}/api/tags` — local Ollama.
pub fn probe_ollama(base_url: &str) -> Result<u128, String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|e: reqwest::Error| e.to_string())?;
    let base = base_url.trim_end_matches('/');
    let url = format!("{base}/api/tags");
    let start = Instant::now();
    let resp = client.get(&url).send().map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }
    Ok(start.elapsed().as_millis())
}

/// Anthropic: `GET` `/v1/models` on the configured `base_url` (defaults to `https://api.anthropic.com`).
pub fn probe_anthropic(base_url: &str, api_key: &str) -> Result<u128, String> {
    use reqwest::StatusCode;

    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|e: reqwest::Error| e.to_string())?;
    let base = base_url.trim_end_matches('/');
    let url = format!("{base}/v1/models");
    let start = Instant::now();
    let resp = client
        .get(&url)
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .send()
        .map_err(|e| e.to_string())?;
    let status = resp.status();
    if matches!(status, StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN) {
        return Err(format!("HTTP {status} (check API key)"));
    }
    if !(status.is_success()
        || status == StatusCode::NOT_FOUND
        || status == StatusCode::METHOD_NOT_ALLOWED)
    {
        return Err(format!("HTTP {status}"));
    }
    Ok(start.elapsed().as_millis())
}

/// Google Generative Language API — list models.
pub fn probe_gemini(api_key: &str) -> Result<u128, String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|e: reqwest::Error| e.to_string())?;
    let start = Instant::now();
    let url = "https://generativelanguage.googleapis.com/v1beta/models";
    let resp = client.get(url).query(&[("key", api_key)]).send().map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }
    Ok(start.elapsed().as_millis())
}
