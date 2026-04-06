# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] — 2026-04-06

### Added
- Windows service via `windows-service` crate
  - Auto-start on Windows boot
  - Graceful stop on `Stop`/`Shutdown` signals
  - Automatic recovery: 3 restarts × 5 s delay after crash
- Temperature collection via WMI connected to `ROOT\LibreHardwareMonitor`
- Load categorisation: `idle` (0–14 %), `light` (15–39 %), `moderate` (40–74 %), `heavy` (75–100 %)
- SQLite storage via `rusqlite` with `bundled` feature — no external DLL required
- Configurable collection interval: `collect_interval_secs` in `config.toml` (min 60 s, default 300 s)
- Text report: per-sensor trend analysis over 4 windows (24h, 7j, 30j, 90j) with delta alerting (≥ 5 °C triggers summary)
- Scheduled toast notifications (daily / weekly / monthly) via Windows Scheduled Tasks
  - Works around Session 0 isolation: tasks run in the logged-in user's session
  - AUMID `MonitoringAlert.TemperatureMonitor` registered in HKCU by `install.bat`
  - Schedule fully configurable via `config.toml` (time, day-of-week, day-of-month)
- `install.bat` / `update.bat` / `uninstall.bat` batch scripts
  - Configurable install directory and DB path via `config.toml`
  - `update.bat` auto-downloads the latest release from GitHub Releases and resyncs scheduled tasks
- `TemperatureStore` trait + `SqliteStore` implementation (swappable storage backend)
- `ReportSender` trait + `ToastSender` implementation (swappable delivery channel)
- CLI subcommands: `collect`, `watch`, `report`, `service` (install / uninstall / start / stop), `notify` (--daily / --weekly / --monthly)
- 23 unit and integration tests covering aggregation algorithm with dynamic UTC timestamps
- Pre-commit hooks: `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`
- GitHub Actions CI: format + lint + build + test on `windows-latest` on every push to `main`
- GitHub Actions Release: build Windows binary and publish GitHub Release on tag push (`v*`)
- `scripts/release.sh` to automate version bump, CHANGELOG promotion, git tag, and push

---

[Unreleased]: https://github.com/jordanatdown/monitoring-alert/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/jordanatdown/monitoring-alert/releases/tag/v0.1.0
