//! Prepares the dashboard files embedded in the executable.
//!
//! The frontend (`../frontend`, built with `npm run build`) writes a fixed list of
//! files to `../frontend/dist`. They are copied to `OUT_DIR/dashboard` and pulled
//! in with `include_bytes!` by `src/assets.rs`. Without a build of the frontend
//! (plain `cargo test` / `cargo clippy` on a fresh checkout) small placeholders
//! are embedded instead, so Rust development never needs Node. A *release* build
//! without the frontend is refused: it would ship a dashboard that says it is empty.

use std::path::Path;
use std::{env, fs};

const FILES: &[&str] = &[
    "index.html",
    "assets/app.js",
    "assets/app.css",
    "favicon.svg",
];

const PLACEHOLDER_INDEX: &str =
    "<!doctype html><meta charset=\"utf-8\"><title>monitoring-alert</title>\
<h1>monitoring-alert</h1><p>The dashboard was not built into this executable \
(run <code>npm ci &amp;&amp; npm run build</code> in <code>frontend/</code>, then rebuild). \
The JSON API is available under <code>/api/</code>.</p>";

fn main() {
    let manifest = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    let dist = Path::new(&manifest).join("../frontend/dist");
    let out = Path::new(&env::var("OUT_DIR").expect("OUT_DIR")).join("dashboard");

    println!("cargo:rerun-if-changed=../frontend/dist");
    println!("cargo:rerun-if-env-changed=MONITORING_ALERT_ALLOW_NO_FRONTEND");

    let built = dist.join("index.html").is_file();
    let release = env::var("PROFILE").is_ok_and(|p| p == "release");
    if release && !built && env::var_os("MONITORING_ALERT_ALLOW_NO_FRONTEND").is_none() {
        panic!(
            "frontend/dist is missing: run `npm ci && npm run build` in frontend/ before a release \
             build (or set MONITORING_ALERT_ALLOW_NO_FRONTEND=1 to embed a placeholder)."
        );
    }

    for file in FILES {
        let dest = out.join(file);
        fs::create_dir_all(dest.parent().expect("parent")).expect("create dashboard dir");
        let content = fs::read(dist.join(file)).unwrap_or_else(|_| {
            if *file == "index.html" {
                PLACEHOLDER_INDEX.as_bytes().to_vec()
            } else {
                Vec::new()
            }
        });
        fs::write(&dest, content).expect("write dashboard file");
    }
}
