# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Initial project structure for `monitoring-alert` Windows temperature monitoring service
- SQLite embedded database (no external DLL) via `rusqlite` with `bundled` feature
- WMI sensor reading via `wmi` crate connected to `ROOT\LibreHardwareMonitor`
- Load categorization: `idle`, `light`, `moderate`, `heavy` based on CPU %
- CLI interface with `clap`: `collect`, `watch`, `report`, `service` subcommands
- Windows Service integration via `windows-service` crate
  - Auto-start on Windows boot
  - Graceful stop on `Stop`/`Shutdown` events
  - Automatic recovery (3 restarts × 5s delay) after crash
- Text report generation with per-sensor, per-load-category trend analysis
  - 4 time windows: 24h, 7j, 30j, 90j
  - Delta-based alerting (≥5°C on 30j triggers a summary alert)
- Pre-commit hooks: `cargo fmt --check`, `cargo clippy`, `cargo test`
- GitHub Actions workflow: build + test on `windows-latest` on every push to `main`
- `CLAUDE.md` with conventional commits convention and development workflow

## [0.1.0] — YYYY-MM-DD

_Initial release — placeholder, to be filled on first tag._

---

[Unreleased]: https://github.com/jordanatdown/monitoring-alert/compare/HEAD...HEAD
