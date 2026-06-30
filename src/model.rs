use std::{fmt, str::FromStr, time::Duration};

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum HttpMethod {
    #[default]
    Get,
    Post,
    Put,
    Patch,
    Delete,
    Head,
    Options,
}

impl HttpMethod {
    pub const ALL: [Self; 7] = [
        Self::Get,
        Self::Post,
        Self::Put,
        Self::Patch,
        Self::Delete,
        Self::Head,
        Self::Options,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
            Self::Put => "PUT",
            Self::Patch => "PATCH",
            Self::Delete => "DELETE",
            Self::Head => "HEAD",
            Self::Options => "OPTIONS",
        }
    }
}

impl fmt::Display for HttpMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for HttpMethod {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.to_ascii_uppercase().as_str() {
            "GET" => Ok(Self::Get),
            "POST" => Ok(Self::Post),
            "PUT" => Ok(Self::Put),
            "PATCH" => Ok(Self::Patch),
            "DELETE" => Ok(Self::Delete),
            "HEAD" => Ok(Self::Head),
            "OPTIONS" => Ok(Self::Options),
            _ => Err(()),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyValueRow {
    pub enabled: bool,
    pub key: String,
    pub value: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum BodyMode {
    #[default]
    None,
    Json(String),
    Raw(String),
}

impl BodyMode {
    pub fn text(&self) -> &str {
        match self {
            Self::None => "",
            Self::Json(value) | Self::Raw(value) => value,
        }
    }

    pub fn from_editor_text(value: String) -> Self {
        if value.trim().is_empty() {
            Self::None
        } else if serde_json::from_str::<serde_json::Value>(&value).is_ok() {
            Self::Json(value)
        } else {
            Self::Raw(value)
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequestDraft {
    pub name: String,
    pub method: HttpMethod,
    pub url: String,
    pub headers: Vec<KeyValueRow>,
    pub params: Vec<KeyValueRow>,
    pub body: BodyMode,
}

impl Default for RequestDraft {
    fn default() -> Self {
        Self {
            name: "New request".to_string(),
            method: HttpMethod::Get,
            url: "https://httpbin.org/get".to_string(),
            headers: Vec::new(),
            params: Vec::new(),
            body: BodyMode::None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ResponseRecord {
    pub status: u16,
    pub status_text: String,
    pub headers: Vec<KeyValueRow>,
    pub body: String,
    pub pretty_body: String,
    pub duration: Duration,
    pub size_bytes: usize,
}

impl ResponseRecord {
    pub fn duration_ms(&self) -> u128 {
        self.duration.as_millis()
    }
}
