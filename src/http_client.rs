use std::time::Instant;

use anyhow::{Result, anyhow};
use reqwest::{
    blocking::Client,
    header::{CONTENT_TYPE, HeaderMap, HeaderName, HeaderValue},
};

use crate::model::{BodyMode, HttpMethod, KeyValueRow, RequestDraft, ResponseRecord};

pub fn send_request(client: Client, draft: RequestDraft) -> Result<ResponseRecord> {
    let method = match draft.method {
        HttpMethod::Get => reqwest::Method::GET,
        HttpMethod::Post => reqwest::Method::POST,
        HttpMethod::Put => reqwest::Method::PUT,
        HttpMethod::Patch => reqwest::Method::PATCH,
        HttpMethod::Delete => reqwest::Method::DELETE,
        HttpMethod::Head => reqwest::Method::HEAD,
        HttpMethod::Options => reqwest::Method::OPTIONS,
    };

    if draft.url.trim().is_empty() {
        return Err(anyhow!("URL is required"));
    }

    let mut builder = client.request(method, draft.url.trim());
    let headers = request_headers(&draft.headers)?;
    if !headers.is_empty() {
        builder = builder.headers(headers);
    }

    let params: Vec<_> = draft
        .params
        .iter()
        .filter(|row| row.enabled && !row.key.trim().is_empty())
        .map(|row| (row.key.as_str(), row.value.as_str()))
        .collect();
    if !params.is_empty() {
        builder = builder.query(&params);
    }

    match &draft.body {
        BodyMode::None => {}
        BodyMode::Json(body) => {
            builder = builder
                .header(CONTENT_TYPE, "application/json")
                .body(body.clone());
        }
        BodyMode::Raw(body) => {
            builder = builder.body(body.clone());
        }
    }

    let started = Instant::now();
    let response = builder.send()?;
    let duration = started.elapsed();
    let status = response.status();
    let status_text = status.canonical_reason().unwrap_or("").to_string();
    let headers = response_headers(response.headers());
    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("")
        .to_string();
    let bytes = response.bytes()?;
    let size_bytes = bytes.len();
    let body = String::from_utf8_lossy(&bytes).to_string();
    let pretty_body = pretty_body(&body, &content_type);

    Ok(ResponseRecord {
        status: status.as_u16(),
        status_text,
        headers,
        body,
        pretty_body,
        duration,
        size_bytes,
    })
}

fn request_headers(rows: &[KeyValueRow]) -> Result<HeaderMap> {
    let mut headers = HeaderMap::new();
    for row in rows {
        if !row.enabled || row.key.trim().is_empty() {
            continue;
        }
        let name = HeaderName::from_bytes(row.key.trim().as_bytes())?;
        let value = HeaderValue::from_str(row.value.trim())?;
        headers.insert(name, value);
    }
    Ok(headers)
}

fn response_headers(headers: &HeaderMap) -> Vec<KeyValueRow> {
    headers
        .iter()
        .map(|(key, value)| KeyValueRow {
            enabled: true,
            key: key.to_string(),
            value: value.to_str().unwrap_or("<binary>").to_string(),
        })
        .collect()
}

fn pretty_body(body: &str, content_type: &str) -> String {
    if (content_type.contains("json") || body.trim_start().starts_with(['{', '[']))
        && let Ok(value) = serde_json::from_str::<serde_json::Value>(body)
        && let Ok(pretty) = serde_json::to_string_pretty(&value)
    {
        return pretty;
    }

    body.to_string()
}
