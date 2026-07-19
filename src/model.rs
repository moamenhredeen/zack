use std::time::Duration;

use opencollection::HttpResponseHeader;

/// The HTTP methods offered in the method picker.
///
/// OpenCollection types `method` as a free-form string, so this is a UI
/// convenience only — a request may carry a method that is not in this list
/// and it round-trips untouched.
pub const METHODS: [&str; 7] = ["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS"];

/// A response is not part of the OpenCollection document model — it exists
/// only for the lifetime of the window — so it stays a local type.
#[derive(Clone, Debug)]
pub struct ResponseRecord {
    pub status: u16,
    pub status_text: String,
    pub headers: Vec<HttpResponseHeader>,
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
