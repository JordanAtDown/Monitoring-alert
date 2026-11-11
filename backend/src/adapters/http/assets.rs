//! The dashboard files, embedded in the executable (see `build.rs`).

pub struct Asset {
    pub body: &'static [u8],
    pub mime: &'static str,
}

macro_rules! embedded {
    ($name:literal) => {
        include_bytes!(concat!(env!("OUT_DIR"), "/dashboard/", $name)) as &'static [u8]
    };
}

/// Looks up a file by its URL path (`/`, `/assets/app.js`…).
pub fn get(path: &str) -> Option<Asset> {
    let (body, mime) = match path {
        "/" | "/index.html" => (embedded!("index.html"), "text/html; charset=utf-8"),
        "/assets/app.js" => (embedded!("assets/app.js"), "text/javascript; charset=utf-8"),
        "/assets/app.css" => (embedded!("assets/app.css"), "text/css; charset=utf-8"),
        "/favicon.svg" => (embedded!("favicon.svg"), "image/svg+xml"),
        _ => return None,
    };
    Some(Asset { body, mime })
}
