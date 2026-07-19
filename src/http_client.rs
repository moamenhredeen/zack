use std::time::Instant;

use anyhow::{Result, anyhow};
use opencollection::{
    HttpBodyOrVariants, HttpRequest, HttpRequestBody, HttpRequestHeader, HttpRequestParam,
    HttpResponseHeader, ParamType,
};
use reqwest::{
    blocking::{Client, RequestBuilder},
    header::{CONTENT_TYPE, HeaderMap, HeaderName, HeaderValue},
};

use crate::model::ResponseRecord;

pub fn send_request(client: Client, request: HttpRequest) -> Result<ResponseRecord> {
    let details = request
        .http
        .as_ref()
        .ok_or_else(|| anyhow!("request has no http details"))?;

    let method = details.method.as_deref().unwrap_or("GET").trim();
    let method = reqwest::Method::from_bytes(method.to_ascii_uppercase().as_bytes())
        .map_err(|_| anyhow!("`{method}` is not a valid HTTP method"))?;

    let url = details.url.as_deref().unwrap_or("").trim();
    if url.is_empty() {
        return Err(anyhow!("URL is required"));
    }

    let params = details.params.as_deref().unwrap_or_default();
    let url = substitute_path_params(url, params);

    let mut builder = client.request(method, url);

    let headers = request_headers(details.headers.as_deref().unwrap_or_default())?;
    if !headers.is_empty() {
        builder = builder.headers(headers);
    }

    let query: Vec<_> = params
        .iter()
        .filter(|param| is_enabled(param.disabled) && param.param_type == ParamType::Query)
        .filter(|param| !param.name.trim().is_empty())
        .map(|param| (param.name.as_str(), param.value.as_str()))
        .collect();
    if !query.is_empty() {
        builder = builder.query(&query);
    }

    if let Some(body) = details.body.as_ref().and_then(selected_body) {
        builder = apply_body(builder, body)?;
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

/// The body to send when a request carries several named variants.
///
/// Falls back to the first variant when none is marked selected, so a request
/// with variants always sends something rather than nothing.
pub fn selected_body(body: &HttpBodyOrVariants) -> Option<&HttpRequestBody> {
    match body {
        HttpBodyOrVariants::Body(body) => Some(body),
        HttpBodyOrVariants::Variants(variants) => variants
            .iter()
            .find(|variant| variant.selected == Some(true))
            .or_else(|| variants.first())
            .map(|variant| &variant.body),
    }
}

fn apply_body(builder: RequestBuilder, body: &HttpRequestBody) -> Result<RequestBuilder> {
    let (content_type, data) = match body {
        HttpRequestBody::Json { data } => ("application/json", data),
        HttpRequestBody::Text { data } => ("text/plain", data),
        HttpRequestBody::Xml { data } => ("application/xml", data),
        HttpRequestBody::Sparql { data } => ("application/sparql-query", data),
        HttpRequestBody::FormUrlEncoded { data } => {
            let fields: Vec<_> = data
                .iter()
                .filter(|field| is_enabled(field.disabled))
                .map(|field| (field.name.as_str(), field.value.as_str()))
                .collect();
            return Ok(builder.form(&fields));
        }
        HttpRequestBody::MultipartForm { .. } => {
            return Err(anyhow!("multipart bodies are not supported yet"));
        }
        HttpRequestBody::File { .. } => {
            return Err(anyhow!("file bodies are not supported yet"));
        }
    };

    if data.trim().is_empty() {
        return Ok(builder);
    }
    Ok(builder
        .header(CONTENT_TYPE, content_type)
        .body(data.clone()))
}

/// Fills `:name` and `{name}` placeholders from the request's path params.
fn substitute_path_params(url: &str, params: &[HttpRequestParam]) -> String {
    params
        .iter()
        .filter(|param| is_enabled(param.disabled) && param.param_type == ParamType::Path)
        .fold(url.to_string(), |url, param| {
            url.replace(&format!(":{}", param.name), &param.value)
                .replace(&format!("{{{}}}", param.name), &param.value)
        })
}

fn request_headers(headers: &[HttpRequestHeader]) -> Result<HeaderMap> {
    let mut map = HeaderMap::new();
    for header in headers {
        if !is_enabled(header.disabled) || header.name.trim().is_empty() {
            continue;
        }
        let name = HeaderName::from_bytes(header.name.trim().as_bytes())?;
        let value = HeaderValue::from_str(header.value.trim())?;
        map.insert(name, value);
    }
    Ok(map)
}

fn response_headers(headers: &HeaderMap) -> Vec<HttpResponseHeader> {
    headers
        .iter()
        .map(|(name, value)| HttpResponseHeader {
            name: name.to_string(),
            value: value.to_str().unwrap_or("<binary>").to_string(),
        })
        .collect()
}

fn is_enabled(disabled: Option<bool>) -> bool {
    disabled != Some(true)
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
