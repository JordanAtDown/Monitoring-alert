---
name: release
description: Publier une nouvelle version (SemVer, CHANGELOG, tag, GitHub Release). Procédure manuelle, à ne lancer que sur demande explicite.
disable-model-invocation: true
---

# Publier une version

Publier pousse un tag et déclenche une release publique : ne le faire que si l'utilisateur le demande.

1. Être sur `main`, arbre propre, `git pull origin main`.
2. Choisir la version ([SemVer](https://semver.org/lang/fr/)) : `MAJOR` pour un changement incompatible
   (un commit avec `BREAKING CHANGE`), `MINOR` pour une fonctionnalité, `PATCH` pour une correction.
3. Vérifier que `## [Unreleased]` du `CHANGELOG.md` décrit tout ce qui change (c'est lui qui devient les
   notes de la release), et que le README est à jour.
4. Vérifier la CI sur `main` (verte) et que le frontend se construit : `npm ci && npm run build` dans
   `frontend/`.
5. Lancer `./scripts/release.sh X.Y.Z`. Il met à jour `backend/Cargo.toml`, promeut `[Unreleased]` en
   `[X.Y.Z]`, lance `cargo test`, commit `chore(release): bump version to X.Y.Z`, crée le tag annoté
   `vX.Y.Z` et pousse le commit et le tag.
6. `release.yml` construit le frontend puis l'exécutable et publie l'archive. Suivre l'exécution
   (`gh run watch`) et vérifier le contenu de l'archive publiée (voir `.claude/rules/ci.md`).

Ne jamais déplacer ni supprimer un tag publié.
