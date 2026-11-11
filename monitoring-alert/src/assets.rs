//! Static files of the dashboard.

use std::borrow::Cow;

pub struct Asset {
    pub body: Cow<'static, [u8]>,
    pub mime: &'static str,
}

const PLACEHOLDER: &str = "<!doctype html><meta charset=\"utf-8\"><title>monitoring-alert</title>\
<h1>monitoring-alert</h1><p>Dashboard not built yet. The JSON API is available under /api/.</p>";

/// Looks up a file by its URL path (`/`, `/assets/app.js`…).
pub fn get(path: &str) -> Option<Asset> {
    match path {
        "/" | "/index.html" => Some(Asset {
            body: Cow::Borrowed(PLACEHOLDER.as_bytes()),
            mime: "text/html; charset=utf-8",
        }),
        _ => None,
    }
}
