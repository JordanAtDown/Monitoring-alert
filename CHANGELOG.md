# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.2.0] — 2026-04-08

### Changed
- Répertoire de données déplacé de `C:\ProgramData\MonitoringAlert\` vers
  `%LOCALAPPDATA%\Programs\MonitoringAlert\` (`C:\Users\<user>\AppData\Local\Programs\MonitoringAlert\`)
  — config.toml, scripts et base de données sont désormais dans le profil utilisateur

## [1.1.0] — 2026-04-08

### Changed
- SQLite passe de `journal_mode = WAL` à `journal_mode = DELETE` — plus de
  fichiers `temperatures.db-shm` / `temperatures.db-wal` à côté de la base

## [1.0.9] — 2026-04-08

### Fixed
- Formatage `cargo fmt` sur `service.rs` (suite du correctif v1.0.8)

## [1.0.8] — 2026-04-08

### Fixed
- `service stop` : ne retourne plus d'erreur si le service est déjà arrêté —
  vérification de l'état avant d'envoyer le signal STOP
- `uninstall.bat` : l'étape d'arrêt est ignorée si le service est déjà arrêté,
  évitant le timeout inutile de 3 secondes et l'erreur silencieuse

## [1.0.7] — 2026-04-08

### Fixed
- `service uninstall` : erreur `Accès refusé` lors du `query_status` — le handle
  était ouvert avec `DELETE | STOP` sans `QUERY_STATUS` ; ajout de `QUERY_STATUS`
  dans les droits d'accès demandés

## [1.0.6] — 2026-04-08

### Changed
- Fichiers de log renommés de `monitoring-alert.log.YYYY-MM-DD` en
  `monitoring-alert-YYYY-MM-DD.log` — format plus lisible et trié
  correctement par l'explorateur Windows

## [1.0.5] — 2026-04-08

### Fixed
- `uninstall.bat` et `update.bat` : erreur `{ était inattendu` — mêmes causes
  que `install.bat` v1.0.2/1.0.3 ; réécriture de tous les blocs `for /f`
  PowerShell en commandes sur une seule ligne avec `$q=[char]34`
- `uninstall.bat` et `update.bat` : ajout de `pause` avant chaque `exit /b 1`
  et en fin de script pour éviter la fermeture immédiate en cas d'erreur

## [1.0.4] — 2026-04-08

### Fixed
- `install.bat` : création automatique du dossier parent de `db_path` s'il
  n'existe pas — le service échouait au premier démarrage si le répertoire
  de la base de données n'avait pas été créé manuellement

## [1.0.3] — 2026-04-08

### Fixed
- `install.bat` : les variables `install_dir` et `db_path` restaient vides —
  CMD cassait la commande PowerShell en interprétant `""` à l'intérieur des
  backticks `for /f` ; remplacement par `$q=[char]34` pour construire le
  regex sans guillemets dans la ligne CMD

## [1.0.2] — 2026-04-08

### Fixed
- `install.bat` : erreur `{ inattendu` — les blocs `for /f` PowerShell
  multi-lignes avec `^` faisaient interpréter le `{` du code PowerShell
  comme syntaxe batch ; réécriture en commandes sur une seule ligne

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

[Unreleased]: https://github.com/JordanAtDown/monitoring-alert/compare/v1.2.0...HEAD
[1.2.0]: https://github.com/JordanAtDown/monitoring-alert/compare/v1.1.0...v1.2.0
[1.1.0]: https://github.com/JordanAtDown/monitoring-alert/compare/v1.0.9...v1.1.0
[1.0.9]: https://github.com/JordanAtDown/monitoring-alert/compare/v1.0.8...v1.0.9
[1.0.8]: https://github.com/JordanAtDown/monitoring-alert/compare/v1.0.7...v1.0.8
[1.0.7]: https://github.com/JordanAtDown/monitoring-alert/compare/v1.0.6...v1.0.7
[1.0.6]: https://github.com/JordanAtDown/monitoring-alert/compare/v1.0.5...v1.0.6
[1.0.5]: https://github.com/JordanAtDown/monitoring-alert/compare/v1.0.4...v1.0.5
[1.0.4]: https://github.com/JordanAtDown/monitoring-alert/compare/v1.0.3...v1.0.4
[1.0.3]: https://github.com/JordanAtDown/monitoring-alert/compare/v1.0.2...v1.0.3
[1.0.2]: https://github.com/JordanAtDown/monitoring-alert/compare/v1.0.1...v1.0.2
[1.0.1]: https://github.com/JordanAtDown/monitoring-alert/compare/v1.0.0...v1.0.1
[1.0.0]: https://github.com/JordanAtDown/monitoring-alert/compare/v0.1.0...v1.0.0
[0.1.0]: https://github.com/JordanAtDown/monitoring-alert/releases/tag/v0.1.0
