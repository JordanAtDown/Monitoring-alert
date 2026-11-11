---
paths:
  - ".github/**"
---

# CI et release (GitHub Actions)

- `ci.yml` (push sur `main`, PR vers `main`) : `cargo fmt --check`, `cargo clippy -- -D warnings`,
  `cargo test` sur `windows-latest`, plus un job frontend (`npm ci`, `npm run build`) sur Linux.
  Tout doit être vert.
- `release.yml` (tag `vX.Y.Z`) : lint, **build du frontend puis** `cargo build --release` (le tableau de
  bord est embarqué : sans lui `build.rs` refuse le build), tests, puis publication de l'archive
  `monitoring-alert-vX.Y.Z-windows-x64.zip` avec les notes extraites de `CHANGELOG.md`.
- Le contenu de l'archive est la liste du `cp` de l'étape « Package installer archive » : la tenir
  synchronisée avec `scripts/` (aujourd'hui : exe, `config.toml`, `install.bat`, `update.bat`,
  `uninstall.bat`, `Register-Tasks.ps1`).
- Les actions utilisées sont épinglées sur une version majeure.
