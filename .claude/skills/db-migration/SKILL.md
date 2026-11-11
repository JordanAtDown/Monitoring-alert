---
name: db-migration
description: Changer le schéma SQLite (nouvelle table ou colonne, nouvel agrégat) sans jamais perdre la base de l'utilisateur.
---

# Changer le schéma de la base

La base de l'utilisateur contient des mois d'historique et ne se reconstruit pas : un changement de schéma
est une **migration automatique, sauvegardée, testée sur une vraie base de l'ancienne version**.

Fichier : `backend/src/adapters/sqlite/mod.rs` (`SCHEMA_VERSION`, `init_db`, `migrate`).

1. **Incrémenter `SCHEMA_VERSION`** et documenter la version dans le commentaire au-dessus.
2. Ajouter les `CREATE TABLE/INDEX IF NOT EXISTS` au schéma d'`init_db` (une base neuve doit naître directement
   à jour).
3. Dans `migrate` : exécuter la migration **dans une transaction** (`unchecked_transaction`), puis
   `PRAGMA user_version = N`. Elle ne doit s'exécuter qu'une fois (testé).
4. **Sauvegarde avant** : `backup_before_migration` écrit `<db>.v1.bak` (`VACUUM INTO`) si la base a des
   données ; ne jamais écraser une sauvegarde existante. Adapter le suffixe (`.vN.bak`) pour une nouvelle version.
5. Ne jamais supprimer de données brutes sans les avoir agrégées dans la même transaction (comme la purge).
6. Si la forme des données lues par le tableau de bord change : `application/ports.rs` (`ReadModel`),
   `application/queries.rs`, `frontend/src/api.ts` ensemble.
7. **Tests** dans `backend/src/tests/sqlite.rs` : créer une base de la version précédente **à la main** (comme
   `create_v1_db`), l'ouvrir avec `init_db`, vérifier le contenu migré, la sauvegarde, que la migration ne
   se rejoue pas, et qu'une base neuve n'a pas besoin de sauvegarde.
8. Mettre à jour la section « Base de données » du README et le CHANGELOG.

Les anciennes versions du logiciel n'ouvrent pas une base plus récente sans risque : le dire dans le CHANGELOG.
