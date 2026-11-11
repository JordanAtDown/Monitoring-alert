---
name: add-config-key
description: Ajouter, renommer ou retirer une clé de config.toml (backend + modèle livré + migration + doc + test).
---

# Ajouter / renommer / retirer une clé de configuration

Une clé de configuration touche toujours les mêmes endroits. Les faire **tous**, dans le même commit :

1. `backend/src/adapters/config.rs`
   - champ dans `AppConfig` (documenté) et dans `RawConfig` ;
   - valeur par défaut et bornes dans `from_raw` ;
   - clé affichée dans `render_config` (c'est le modèle écrit par la migration d'une config 1.x), avec un
     défaut commenté.
2. `scripts/config.toml` : la clé, son commentaire et son défaut (ligne commentée si c'est le défaut).
3. `README.md` : tableau « Configuration » (et la section concernée si le comportement change).
4. `CHANGELOG.md` : sous `## [Unreleased]`.
5. Tests dans `backend/src/tests/config.rs` : valeur lue, défaut, bornes. Le test
   « the_shipped_config_template_… » vérifie déjà que le modèle livré se charge.
6. Si la clé **remplace** une ancienne : ajouter l'ancienne à `OBSOLETE_KEYS` (elle sera retirée des
   configs existantes avec sauvegarde) et le dire dans le CHANGELOG. Une clé inconnue est ignorée au chargement
   (pas d'erreur sur une vieille config).
7. Si la clé est lue par le démon : l'utilisateur doit faire `monitoring-alert.exe restart` — le mentionner
   dans la doc de la clé.

Valider : `cargo test config` puis les deux `cargo clippy`.
