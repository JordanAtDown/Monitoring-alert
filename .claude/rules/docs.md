---
paths:
  - "README.md"
  - "CHANGELOG.md"
  - "installer/config.toml"
---

# Documentation

La documentation est mise à jour **dans le même commit** que le comportement qu'elle décrit (ou dans un
commit `docs` juste après pour un changement mineur).

| Fichier | À mettre à jour si… |
|---|---|
| `README.md` | changement visible : installation, migration, configuration, CLI, tableau de bord, calcul de la dérive, base de données |
| `installer/config.toml` | une clé de configuration est ajoutée, retirée ou change de défaut (un test vérifie qu'il se charge et ne contient pas de clé obsolète) |
| `CHANGELOG.md` | à chaque changement notable, sous `## [Unreleased]` ; promu en version par `tools/release.sh` |

- README et CHANGELOG sont en **français**. Le README dit ce qui est vrai **aujourd'hui**, pas l'historique
  (l'historique est dans le CHANGELOG et git).
- Ne pas affirmer dans la doc ce qui n'a pas été vérifié (mémoire, performances, comportement Windows non
  testé) : le dire, ou le qualifier.
