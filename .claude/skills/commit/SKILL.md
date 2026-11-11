---
name: commit
description: Faire un commit de ce projet — format Conventional Commits, atomicité, vérifications des hooks, documentation à jour, push sur main. À utiliser dès qu'il faut committer ou pousser.
---

# Faire un commit

1. **Synchroniser** : `git pull origin main` si ce n'est pas déjà fait dans cette session.
2. **Un commit = une intention.** Ne pas mélanger un `fix` et un `feat`, ni un refactor et un changement
   de comportement. Plusieurs petits commits clairs valent mieux qu'un gros. Pas de WIP sur `main`.
   Chaque commit **compile et passe les tests**.
3. **Documentation** (voir `.claude/rules/docs.md`) : un `feat` ou `fix` visible met à jour README /
   `installer/config.toml` / CHANGELOG dans le même commit, ou dans un commit `docs` immédiatement après.
4. **Ajouter les fichiers explicitement** (`git add <chemins>`) — jamais un `git add -A` à l'aveugle quand le
   dossier contient des fichiers non liés (`frontend/node_modules`, `dist`, sorties de test…). Le hook
   `pre-commit` ne re-stage que les fichiers **déjà indexés** qu'il a reformatés : un fichier modifié mais
   non indexé reste hors du commit. Vérifier `git show --stat HEAD` après le commit.
5. **Message** :

```
<type>(<scope>): <description courte à l'impératif, en anglais, ≤ 100 caractères>

<corps : le POURQUOI, ce qui change pour l'utilisateur, les compromis>

<footer : BREAKING CHANGE: …, closes #n>
Co-Authored-By: …   (si le système en impose une)
```

| Type | Quand |
|---|---|
| `feat` | nouvelle fonctionnalité visible |
| `fix` | correction de bug (avec un test de non-régression) |
| `docs` | documentation uniquement |
| `style` | formatage, sans changement de logique |
| `refactor` | réécriture interne, comportement inchangé |
| `perf` | performances |
| `test` | tests uniquement |
| `chore` | outillage, installateur, dépendances |
| `build` | `Cargo.toml`, `build.rs`, build du frontend |
| `ci` | GitHub Actions, hooks |

Scopes usuels : `db`, `server`, `web`, `trends`, `install`, `config`, `collector`, `installer`, `deps`,
`crate`, `backend`. Un changement incompatible : `!` après le type/scope **et** un footer
`BREAKING CHANGE:`.

Valides : `feat(server): serve a read-only JSON API` · `fix(db): handle a missing directory` ·
`chore(deps): bump rusqlite to 0.32` · `ci: build the frontend before the release`.
Invalides : `fix stuff` · `WIP` · `update` · `feat: many things at once`.

6. **Hooks** : `pre-commit` lance `cargo fmt`, `cargo clippy -D warnings` et `cargo test` ; `commit-msg` vérifie
   le format. Un échec se **corrige** : on ne passe pas `--no-verify`.
   Si le commit touche du code `cfg(windows)`, lancer aussi
   `cargo clippy --target x86_64-pc-windows-gnu --all-targets -- -D warnings`.
7. **Pousser** : `git push origin main` juste après le commit (jamais de branche ni de PR).
