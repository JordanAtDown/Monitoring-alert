---
paths:
  - "backend/**"
---

# Backend (Rust)

## Architecture hexagonale simplifiée

```
domain/       pur : modèle, catégories de charge, calcul de dérive (trends), composants du PC (components),
              vocabulaire (wording), pages du tableau de bord (dashboard/). Aucune E/S.
application/  cas d'usage + ports (traits dans ports.rs) : usecases/ (un fichier par action), collect, monitor,
              install, status, clock
adapters/     technique : sqlite, lhm (HTTP), http (serveur, assets, endpoints/ : un fichier par endpoint), config,
              logging, windows, pe, deploy
daemon.rs     racine de composition : branche les adapters sur les cas d'usage
```

- Dépendances **vers l'intérieur uniquement** : adapters → application → domain. `domain` ne connaît
  rien du crate ; `application` ne connaît que `domain`. `tests/architecture.rs` échoue sinon, et interdit
  aussi `rusqlite`, `ureq`, `tiny_http`, `std::fs`, `std::net`, `std::process`, `std::env`… dans
  `domain/` et `application/`.
- Un nouveau besoin vers l'extérieur = un **port** (trait) dans `application/ports.rs`, implémenté dans
  `adapters/`, avec un faux dans les tests. Pas de trait « au cas où » : il en faut une vraie raison
  (second implémenteur, ou test sans E/S).
- **Un endpoint = un fichier = un cas d'usage.** `adapters/http/endpoints/<nom>.rs` lit la requête, appelle un seul
  `usecases::<action>::execute` (nommé par ce qu'il fait : `get_sensor`, `list_series`…) et sérialise. Le cas d'usage
  lit par les ports puis appelle le code métier du domaine. Seuls les endpoints appellent les cas d'usage ;
  `tests/architecture.rs` le vérifie. Ajouter une route = un endpoint + un cas d'usage + (si besoin) du code de domaine.
- **Le backend décide, le frontend affiche** : regroupement par composant, verdicts, phrases, nombres mis en forme
  (`42,3 °C`), tout vient de l'API (`/api/overview`…). Le frontend n'a ni seuil ni vocabulaire métier.
- L'heure et la date passent par `application/clock.rs`.

## Qualité

- `cargo clippy ... -- -D warnings` doit passer **sur Linux et sur `x86_64-pc-windows-gnu`** : du code
  `#[cfg(windows)]` ne se compile pas sur Linux, et inversement du code mort peut n'apparaître que sur
  une cible. Lancer les deux avant de committer du code touchant `cfg`.
- Pas de `unwrap()`/`expect()` hors tests et hors invariants démontrables (alors `expect` avec message).
- Erreurs : `anyhow` avec `.context()`. Les messages destinés à l'utilisateur (installation, `check`)
  sont en français, les logs `tracing` en anglais.
- Les sorties de commandes Windows sont localisées : on regarde le **code de retour**, jamais le texte.
- Les crates avec feature `bundled` sont préférées (pas de DLL externe).

## Tests

Les tests sont dans `src/tests/`, un fichier par module, groupés par couche dans `tests/mod.rs`.

| Zone | À couvrir au minimum |
|---|---|
| `domain` | `load_category` (toutes les frontières), `trends` (référence, médiane glissante, pente, historique insuffisant) |
| `sqlite` | init, insert, requêtes, `rollup`, `purge` par jours entiers, **migration v1 → v2** depuis une vraie base v1, sauvegarde |
| `application` | `collect_and_store` et la boucle avec une **fausse source** ; `install`/`uninstall` avec un **faux hôte** (ordre des étapes, abandon si l'ancien service ne peut pas être supprimé) |
| `http` | routes avec une base fixture, garde-fous (méthode, `Host`), un test de bout en bout sur un vrai socket |
| `config`, `deploy`, `pe` | chargement et limites, migration d'une config v1, déploiement, patch du sous-système PE |

- Une base de test se crée dans un `tempfile::tempdir()`, ou `:memory:`. Des insertions en masse se font
  **dans une transaction** (sinon un fsync par ligne rend les tests lents).
- Un test de non-régression accompagne chaque `fix`.
