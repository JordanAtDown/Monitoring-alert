---
paths:
  - ".github/**"
---

# CI et release (GitHub Actions)

- `ci.yml` (push sur `main`, PR vers `main`) : `cargo fmt --check`, `cargo clippy -- -D warnings`,
  `cargo test` sur `windows-latest`, un job frontend (`npm ci`, `npm run build`) sur Linux, et un job
  `installer` qui compile `installer/monitoring.iss` avec Inno Setup (Chocolatey) : c'est la seule
  vérification du script. Tout doit être vert.
- `release.yml` (tag `vX.Y.Z`) : lint, **build du frontend puis** `cargo build --release` (le tableau de
  bord est embarqué : sans lui `build.rs` refuse le build), tests, puis un job `installer` (compile
  `monitoring_X.Y.Z.exe` à partir de l'exécutable construit), puis publication de l'installateur
  (seul fichier téléchargeable) avec les notes extraites de `CHANGELOG.md`. Il n'y a plus d'archive zip.
- Les actions utilisées sont épinglées sur une version majeure.
