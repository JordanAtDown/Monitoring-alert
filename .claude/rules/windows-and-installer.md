---
paths:
  - "installer/**"
  - "backend/src/adapters/windows.rs"
  - "backend/src/adapters/deploy.rs"
  - "backend/src/adapters/pe.rs"
  - "backend/src/application/install.rs"
---

# Installation, code Windows, migration

Le code Windows **ne peut pas être exécuté sur la machine de développement** (Linux/WSL) : on le garde
fin, et on le vérifie par `cargo clippy --target x86_64-pc-windows-gnu`, par les tests de
`application/install.rs` (faux hôte) et par `tests/installer.rs`.

## Principe : la logique est en Rust, l'installateur est mince

- **Installateur Inno Setup** (`installer/monitoring-alert.iss`, voie principale) : il copie `monitoring-alert.exe`
  et délègue à `monitoring-alert.exe install --in-place` ; à la désinstallation à `uninstall`. Par utilisateur
  (`PrivilegesRequired=lowest`) : l'administrateur n'est demandé que si un service 1.x est présent. Le fichier
  `.iss` est en **UTF-8 avec BOM**, et **aucune ligne n'y commence par `#`** hors directives (le préprocesseur
  la lirait comme une directive : utiliser la constante `NL`). Il se compile en CI (`ISCC`), c'est la seule
  validation possible : la regarder après chaque modification (`gh run list`).
  La page « Base de données » lit `db locate --porcelain` (clés `path`, `default`, `summary`) : changer ces
  clés = changer le `.iss` (un test les rapproche).
- Toute logique d'installation vit dans `application/install.rs` (cas d'usage), `adapters/windows.rs`
  (système) et `adapters/deploy.rs` (fichiers) : testable, sans pièges de guillemets de `.bat`.
- On ne parse jamais le texte d'une commande Windows (localisé en français) : codes de retour seulement.
- Il n'y a **plus de scripts `.bat`** ni d'archive portable : l'installateur est la seule voie.

## Garanties à ne jamais casser

1. **La base de données de l'utilisateur n'est jamais supprimée** par l'installation ni par la migration.
   L'installateur ne supprime le dossier de données que sur confirmation explicite
   (non par défaut) ; une base située ailleurs que dans ce dossier n'est jamais supprimée.
2. **Deux collecteurs n'écrivent jamais dans la même base** : si l'ancien service ne peut pas être
   supprimé, l'installation s'arrête avant de déployer ou de démarrer quoi que ce soit.
3. `install` est **idempotent** (nouvelle machine, machine 1.x, ou relance après mise à jour).
4. Une configuration existante n'est **jamais écrasée** ; une config 1.x est réécrite avec sauvegarde
   `config.toml.v1.bak`.
5. Le nettoyage d'une 1.x reste dans `install` tant que des installations 1.x peuvent exister : service,
   tâches planifiées, AUMID, `apply-config.bat`, `Register-Tasks.ps1`, et réécriture de `config.toml`.
6. `monitoring-alert.exe` garde son nom (l'installateur, `Run` et la doc le référencent).

## `monitoring-daemon.exe`

C'est une **copie** de `monitoring-alert.exe` dont le sous-système PE est passé à GUI (pas de fenêtre
console), faite par `install` (`adapters/pe.rs`). Elle est relancée par la clé `Run` de l'utilisateur
(`"<chemin>" serve`). Le chemin est entre guillemets dans la clé.
