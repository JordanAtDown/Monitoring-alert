# CLAUDE.md — Workflow de développement monitoring-alert

Ce fichier définit les conventions et le workflow à respecter pour toutes les
contributions au projet **monitoring-alert**.

---

## Conventional Commits

Tous les commits **doivent** suivre la spécification
[Conventional Commits v1.0](https://www.conventionalcommits.org/en/v1.0.0/).

### Format

```
<type>(<scope>): <description courte>

[corps optionnel]

[footer optionnel : BREAKING CHANGE, closes #issue, etc.]
```

### Types autorisés

| Type | Quand l'utiliser |
|---|---|
| `feat` | Nouvelle fonctionnalité visible par l'utilisateur |
| `fix` | Correction de bug |
| `docs` | Documentation uniquement (README, CHANGELOG, commentaires) |
| `style` | Formatage, espaces, virgules — aucun changement de logique |
| `refactor` | Réécriture interne sans changer le comportement |
| `perf` | Amélioration des performances |
| `test` | Ajout ou modification de tests |
| `chore` | Outillage, CI, dépendances, scripts |
| `build` | Système de build, Cargo.toml |
| `ci` | GitHub Actions, hooks |

### Exemples valides

```
feat(collector): add GPU load tracking in snapshots
fix(db): handle missing ProgramData directory on service start
docs(readme): add LibreHardwareMonitor WMI setup instructions
chore(deps): bump rusqlite to 0.32
ci: add cargo audit step to build workflow
```

### Exemples invalides ❌

```
fix stuff
WIP
update
feat: many things at once
```

---

## Commits atomiques

**Un commit = une intention.**

- Ne pas mélanger un `fix` et un `feat` dans le même commit
- Ne pas committer du code WIP sur `main`
- Préférer plusieurs petits commits clairs à un gros commit flou
- Chaque commit doit compiler et passer les tests (`cargo check` doit réussir)

---

## Branche de travail

**Toujours travailler sur `main`.** Ce projet n'utilise pas de branches de
fonctionnalité ni de Pull Requests.

| Règle | Détail |
|---|---|
| Branche unique | `main` — tout le travail se fait ici |
| Push direct | `git push origin main` après chaque commit |
| Jamais de branche intermédiaire | Pas de `feat/*`, `fix/*`, ni PR |

---

## Workflow de développement

```
1. git pull origin main              — synchroniser avant de commencer
2. Développer en commits atomiques
3. git push -u origin main           — pousser immédiatement sur main
```

> Claude doit systématiquement pousser sur `main` et uniquement sur `main`,
> sans créer de branche intermédiaire ni de Pull Request.

---

## Pre-commit hooks

Les hooks sont dans `.githooks/`. Les activer une fois :

```bash
git config core.hooksPath .githooks
```

Le hook `pre-commit` exécute dans l'ordre :

1. `cargo fmt --check` — vérifie le formatage (sans modifier les fichiers)
2. `cargo clippy -- -D warnings` — interdit tous les warnings Clippy
3. `cargo test` — fait tourner toute la suite de tests

> Le commit est **bloqué** si l'une de ces étapes échoue.

---

## Tests

```bash
# Lancer tous les tests
cargo test

# Lancer un test précis
cargo test nom_du_test

# Avec sortie verbose
cargo test -- --nocapture
```

Les tests doivent couvrir au minimum :
- `collector::load_category` — toutes les plages
- `db` — init, insert, query (tests avec DB en mémoire `:memory:`)
- `report` — vérifier que le rendu ne panique pas sur une DB vide

---

## CI / GitHub Actions

Le workflow `.github/workflows/build.yml` se déclenche sur :
- Tout push sur `main`
- Toute Pull Request ciblant `main`

Étapes du workflow :
1. `cargo fmt --check`
2. `cargo clippy -- -D warnings`
3. `cargo build --release`
4. `cargo test`

> Tout merge sur `main` requiert que la CI soit verte.

---

## Dépendances

- Ne pas ajouter de dépendance sans justification dans la PR
- Préférer les crates avec feature `bundled` / `static` pour éviter les DLL externes
- Vérifier les licences : MIT ou Apache-2.0 requis

---

## Documentation

**À chaque commit, vérifier que la documentation est à jour.**

| Fichier | Mettre à jour si… |
|---|---|
| `monitoring-alert/README.md` | Changement de comportement visible (install, config, CLI, rapport) |
| `scripts/config.toml` | Ajout ou suppression d'une clé de configuration |
| `CHANGELOG.md` | Avant chaque release (toujours) |

> Un commit `feat` ou `fix` qui modifie le comportement visible doit être accompagné
> d'un commit `docs` (ou inclure la mise à jour dans le même commit si elle est mineure).

---

## Versioning

- Suivre [SemVer](https://semver.org/) : `MAJOR.MINOR.PATCH`
- Mettre à jour `CHANGELOG.md` avant chaque release
- Tagger la release : `git tag -s v0.1.0 -m "chore: release v0.1.0"`
