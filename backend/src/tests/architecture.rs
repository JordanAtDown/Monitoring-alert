//! The layering rule of the crate, checked on the source files:
//! adapters → application → domain, never the other way round, and nothing
//! technical (database, network, file system, processes) inside the two inner layers.

use std::fs;
use std::path::{Path, PathBuf};

fn rust_files(dir: &Path) -> Vec<PathBuf> {
    // A layer is a directory, or a single `<name>.rs` file.
    let single = dir.with_extension("rs");
    if single.is_file() {
        return vec![single];
    }
    let mut out = Vec::new();
    for entry in fs::read_dir(dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display())) {
        let path = entry.unwrap().path();
        if path.is_dir() {
            out.extend(rust_files(&path));
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
    out
}

fn src(layer: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join(layer)
}

/// Lines of code (comments and blank lines left out) of every file of a layer.
fn code_lines(layer: &str) -> Vec<(PathBuf, usize, String)> {
    let mut out = Vec::new();
    for file in rust_files(&src(layer)) {
        for (i, line) in fs::read_to_string(&file).unwrap().lines().enumerate() {
            let trimmed = line.trim_start();
            if !trimmed.is_empty() && !trimmed.starts_with("//") {
                out.push((file.clone(), i + 1, line.to_string()));
            }
        }
    }
    out
}

fn assert_none(layer: &str, forbidden: &[&str], why: &str) {
    let offences: Vec<String> = code_lines(layer)
        .into_iter()
        .filter(|(_, _, line)| forbidden.iter().any(|f| line.contains(f)))
        .map(|(file, n, line)| format!("{}:{n}: {}", file.display(), line.trim()))
        .collect();
    assert!(
        offences.is_empty(),
        "{layer}/ must not depend on {why}:\n{}",
        offences.join("\n")
    );
}

const TECHNICAL: &[&str] = &[
    "rusqlite",
    "ureq",
    "tiny_http",
    "windows_service",
    "winreg",
    "std::fs",
    "std::net",
    "std::process",
    "std::env",
    "tracing_subscriber",
    "tracing_appender",
    "toml::",
];

#[test]
fn the_domain_depends_on_nothing_of_the_crate() {
    assert_none(
        "domain",
        &["crate::application", "crate::adapters", "crate::daemon"],
        "the application, the adapters or the composition root",
    );
}

#[test]
fn the_domain_does_no_io() {
    assert_none("domain", TECHNICAL, "technical crates or I/O");
}

#[test]
fn the_application_depends_on_the_domain_only() {
    assert_none(
        "application",
        &["crate::adapters", "crate::daemon"],
        "the adapters or the composition root",
    );
}

#[test]
fn the_application_does_no_technical_io() {
    assert_none("application", TECHNICAL, "technical crates or I/O");
}

#[test]
fn adapters_do_not_know_each_other_except_through_ports() {
    // Each adapter may use the application and the domain, but the sqlite adapter must
    // not reach into the http one and vice versa.
    for (layer, other) in [
        ("adapters/sqlite", "adapters::http"),
        ("adapters/http", "adapters::sqlite"),
        ("adapters/http", "adapters::lhm"),
        ("adapters/lhm", "adapters::sqlite"),
        ("adapters/lhm", "adapters::http"),
    ] {
        assert_none(layer, &[other], other);
    }
}

// ── One endpoint, one use case ────────────────────────────────────────────────

fn endpoint_files() -> Vec<PathBuf> {
    rust_files(&src("adapters/http/endpoints"))
        .into_iter()
        .filter(|f| f.file_name().is_some_and(|n| n != "mod.rs"))
        .collect()
}

#[test]
fn each_endpoint_lives_in_its_own_file_and_calls_exactly_one_use_case() {
    let files = endpoint_files();
    assert!(
        files.len() >= 9,
        "the endpoints are split into one file each"
    );
    for file in files {
        let text = fs::read_to_string(&file).unwrap();
        let calls = text.matches("::execute(").count();
        assert_eq!(
            calls,
            1,
            "{} must call exactly one use case (`usecases::…::execute`)",
            file.display()
        );
        assert!(
            !text.contains("crate::domain::trends") && !text.contains("crate::adapters::sqlite"),
            "{} does the work of its use case",
            file.display()
        );
    }
}

#[test]
fn every_api_route_is_served_by_an_endpoint_file() {
    let router = fs::read_to_string(src("adapters/http/mod.rs")).unwrap();
    let routes = router.matches("=> endpoints::").count();
    assert_eq!(
        routes,
        endpoint_files().len(),
        "one route per endpoint file"
    );
}

#[test]
fn use_cases_are_called_from_the_endpoints_only() {
    for layer in ["adapters", "daemon"] {
        for (file, n, line) in code_lines(layer) {
            let in_endpoints = file.to_string_lossy().contains("http/endpoints");
            assert!(
                in_endpoints || !line.contains("application::usecases"),
                "{}:{n}: only the endpoints call the use cases",
                file.display()
            );
        }
    }
}
