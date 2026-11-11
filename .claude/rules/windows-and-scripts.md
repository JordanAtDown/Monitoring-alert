---
paths:
  - "scripts/**"
  - "backend/src/adapters/windows.rs"
  - "backend/src/adapters/deploy.rs"
  - "backend/src/adapters/pe.rs"
  - "backend/src/application/install.rs"
---

# Installation, scripts Windows, migration

Le code Windows **ne peut pas être exécuté sur la machine de développement** (Linux/WSL) : on le garde
fin, et on le vérifie par `cargo clippy --target x86_64-pc-windows-gnu`, par les tests de
`application/install.rs` (faux hôte) et par `tests/scripts.rs`.

## Principe : la logique est en Rust, les `.bat` sont minces

- `install.bat` appelle `monitoring-alert.exe install`, rien de plus. Toute logique d'installation vit dans
  `application/install.rs` (cas d'usage), `adapters/windows.rs` (système), `adapters/deploy.rs` (fichiers).
- `uninstall.bat` est la seule exception : il n'utilise **que** `sc`, `schtasks`, `reg`, `taskkill` pour
  fonctionner quelle que soit la version installée (1.x ou 2.x) et sans exécutable fonctionnel.
- Dans un `.bat` : **toujours entre guillemets** les chemins (ils contiennent des espaces : c'est déjà
  arrivé), `%` et `!` ne se mélangent pas dans une même variable, ne jamais écraser un `.bat` pendant
  qu'il s'exécute (`update.bat` se relance depuis `%TEMP%`).
- On ne parse jamais le texte d'une commande (localisé en français) : codes de retour seulement.

## Garanties à ne jamais casser

1. **La base de données de l'utilisateur n'est jamais supprimée** par l'installation ni par la migration.
   `uninstall.bat` ne la supprime que sur confirmation explicite.
2. **Deux collecteurs n'écrivent jamais dans la même base** : si l'ancien service ne peut pas être
   supprimé, l'installation s'arrête avant de déployer ou de démarrer quoi que ce soit.
3. `install` est **idempotent** (nouvelle machine, machine 1.x, ou relance après mise à jour).
4. Une configuration existante n'est **jamais écrasée** ; une config 1.x est réécrite avec sauvegarde
   `config.toml.v1.bak`.
5. Le `Register-Tasks.ps1` livré est un **pont de migration** (marqueur
   `MIGRATION_SHIM_MARKER`) : un `update.bat` 1.x déjà installé ne copie que `monitoring-alert.exe` puis
   exécute ce script. Ne pas le supprimer tant que des installations 1.x peuvent exister.
6. `monitoring-alert.exe` garde son nom : l'ancien `update.bat` le cherche dans l'archive.

## `monitoring-daemon.exe`

C'est une **copie** de `monitoring-alert.exe` dont le sous-système PE est passé à GUI (pas de fenêtre
console), faite par `install` (`adapters/pe.rs`). Elle est relancée par la clé `Run` de l'utilisateur
(`"<chemin>" serve`). Le chemin est entre guillemets dans la clé.
