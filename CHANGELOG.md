# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.0.1] — 2026-04-08

### Fixed
- `install.bat` se fermait immédiatement sans message visible en cas d'erreur :
  ajout d'un `pause` avant chaque `exit /b 1` et en fin de script
- `Register-Tasks.ps1` absent de l'archive de release, causant un échec silencieux
  de l'installation des tâches planifiées

## [1.0.0] — 2026-04-07

### Added
- **Fenêtre d'analyse 180j** : comparaison saisonnière (été N vs été N−1) dans le
  rapport détaillé — détecte le vieillissement progressif du matériel
- **Section « Recommandation Maintenance »** dans le rapport, basée sur le pire
  delta observé toutes sondes confondues :
  - Δ 180j ≥ +10 °C → 🔴 Inspection matérielle urgente (changer pâte, inspecter ventilateurs)
  - Δ 90j ≥ +8 °C  → ⚠ Maintenance préventive recommandée (nettoyage + vérification)
  - Δ 30j ≥ +5 °C  → ⚠ Nettoyage conseillé
  - Sinon          → ✓ Aucune action requise
- **Rétention automatique** (`retention_days`, défaut 365 j, minimum 360 j) :
  purge des snapshots anciens au démarrage du service puis toutes les 24 h ;
  le minimum est passé de 180 à 360 j pour couvrir la fenêtre saisonnière 180j
  (180j courant + 180j référence)
- **Logging fichier** via `tracing` + `tracing-appender` : rotation quotidienne
  dans le répertoire de la base de données (`monitoring-alert.log.YYYY-MM-DD`)
- **Niveau de log configurable** (`log_level` dans `config.toml`) :
  `"error"` | `"warn"` | `"info"` (défaut) | `"debug"` | `"trace"`
- **Détection LHM absent** : warning immédiat si aucune sonde lue, escalade
  en `error` après 12 collections consécutives vides (~1 h au rythme par défaut)
- **`monitoring-alert db stats`** : affiche taille disque, nombre de snapshots,
  première et dernière mesure
- **`monitoring-alert db vacuum`** : VACUUM SQLite avec affichage avant/après en MB
- **Avertissement données insuffisantes** dans le rapport : indique combien de
  jours sont enregistrés sur les 360 requis, avec dates de disponibilité graduées
  (90j puis 180j)
- **Charge effective** = `max(cpu_load, gpu_load)` pour la catégorisation :
  les sessions GPU-intensives (jeu : GPU 90 %, CPU 15 %) sont désormais
  correctement classées `heavy` et non `light`
- **`monitoring-alert check`** : diagnostic pré-démarrage en 5 points —
  configuration, base de données, service (installé + actif), LHM/WMI
  (connexion + sondes), AUMID registre (toasts disponibles) ; affiche
  les actions correctives pour chaque point en échec
- **`monitoring-alert service status`** : état du service SCM avec PID si actif
- **`monitoring-alert notify-dry-run`** : envoie des toasts fictifs pour tester
  l'implémentation (AUMID, affichage) sans avoir besoin de données ;
  scénarios disponibles : `stable`, `attention`, `critique`, `multi`, `all`
  (4 s de pause entre chaque) ; `--period daily|weekly|monthly` contrôle le titre

### Fixed
- Les températures GPU en session de jeu étaient comparées à des périodes
  de faible charge CPU au lieu d'autres sessions GPU-intensives
- **Notifications manquées si le PC était éteint** : les tâches planifiées
  sont désormais créées avec `StartWhenAvailable` via `Register-Tasks.ps1`
  (PowerShell). `schtasks.exe` n'expose pas cette option — les rapports sont
  maintenant envoyés dès la prochaine ouverture de session si l'heure prévue
  a été ratée

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
- Text report: per-sensor trend analysis over 4 windows (24h, 7j, 30j, 90j) with delta alerting (≥ 5 °C triggers summary) — extended to 5 windows (+ 180j) in [Unreleased]
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

[Unreleased]: https://github.com/JordanAtDown/monitoring-alert/compare/v1.0.1...HEAD
[1.0.1]: https://github.com/JordanAtDown/monitoring-alert/compare/v1.0.0...v1.0.1
[1.0.0]: https://github.com/JordanAtDown/monitoring-alert/compare/v0.1.0...v1.0.0
[0.1.0]: https://github.com/JordanAtDown/monitoring-alert/releases/tag/v0.1.0
