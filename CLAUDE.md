# CLAUDE.md — monitoring-alert

Suivi long terme de la **dérive des températures** : un processus d'arrière-plan collecte les
capteurs de LibreHardwareMonitor et sert un tableau de bord local. Pas de service Windows, pas de
rapports, pas de notifications. Détails d'usage et d'architecture : [README.md](README.md).

```
backend/    Rust — bibliothèque, CLI (monitoring-alert.exe), démon (copie monitoring-daemon.exe), serveur web
frontend/   TypeScript (Vite + uPlot) — tableau de bord, embarqué dans l'exécutable par backend/build.rs
installer/  monitoring-alert.iss — installateur Inno Setup (voie principale d'installation)
scripts/    version portable : install.bat, update.bat, uninstall.bat ; config.toml (modèle) ; release.sh
```

Le backend est en **architecture hexagonale simplifiée** : `domain/` (pur) ← `application/` (cas d'usage
+ ports) ← `adapters/` (SQLite, LHM, HTTP, Windows…). Les dépendances ne vont que vers l'intérieur,
`tests/architecture.rs` le vérifie. Détail : `.claude/rules/backend.md`.

## Commandes

```bash
# Backend (depuis backend/)
cargo test
cargo clippy --all-targets --all-features -- -D warnings
cargo clippy --target x86_64-pc-windows-gnu --all-targets -- -D warnings   # code cfg(windows), depuis Linux

# Frontend (depuis frontend/)
npm ci && npm run build            # typecheck + build → frontend/dist
```

Le hook `pre-commit` (cargo fmt, clippy, tests) et le hook `commit-msg` (Conventional Commits) sont dans
`.githooks/`. À activer une fois : `git config core.hooksPath .githooks`. Un commit refusé par un hook se
corrige, on ne contourne pas le hook.

## Règles non négociables

- **Branche unique : `main`.** Pas de branche, pas de Pull Request. Toujours `git pull origin main` avant de
  commencer, et `git push origin main` après chaque commit.
- **Commits Conventional Commits, atomiques** (une intention par commit, chacun compile et passe les tests).
  Procédure et exemples : skill `commit`.
- **Documentation à jour dans le même commit** que le comportement qu'elle décrit : `.claude/rules/docs.md`.
- **Aucune dépendance sans justification** (message de commit), licence MIT ou Apache-2.0, privilégier
  les crates `bundled`/statiques. Le frontend reste sans framework.
- **Ne jamais supprimer ni réécrire la base de données d'un utilisateur** : toute évolution du schéma est une
  migration automatique avec sauvegarde (skill `db-migration`).
- Le code Windows ne peut pas être exécuté ici : il est vérifié par `cargo clippy --target
  x86_64-pc-windows-gnu`, des tests sur un faux système hôte, et en le gardant le plus fin possible.

## Où est le reste

| Sujet | Où |
|---|---|
| Conventions backend, tests, architecture | `.claude/rules/backend.md` |
| Conventions frontend et graphiques | `.claude/rules/frontend.md` |
| Scripts `.bat`, installation, migration | `.claude/rules/windows-and-scripts.md` |
| CI et release (GitHub Actions) | `.claude/rules/ci.md` |
| Quelle doc mettre à jour | `.claude/rules/docs.md` |
| Faire un commit | skill `commit` |
| Publier une version (SemVer, tag, CHANGELOG) | skill `release` (manuel) |
| Ajouter une clé de configuration | skill `add-config-key` |
| Changer le schéma SQLite | skill `db-migration` |
