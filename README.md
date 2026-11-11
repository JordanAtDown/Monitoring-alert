# monitoring-alert — Suivi long terme de la dérive des températures

Collecte les températures de la machine toutes les 5 minutes et les affiche sur un
**tableau de bord local** pour voir, sur des mois, si elles dérivent : poussière,
pâte thermique qui sèche, ventilateur qui faiblit.

Il n'envoie **aucune notification** : c'est un outil à regarder, pas une alerte.

```
LibreHardwareMonitor ──HTTP──▶ monitoring-daemon ──▶ temperatures.db
 (admin, serveur :8085)         collecte + serveur web      ▲
                                      │                     │
                                      └──▶ http://127.0.0.1:8787  (navigateur)
```

Un seul processus d'arrière-plan, sans fenêtre, démarré à l'ouverture de session.
Pas de service Windows. De l'ordre de **12 Mo de mémoire** (mesuré sur la version release, sous Linux).

---

## Prérequis

| Logiciel | Notes |
|---|---|
| LibreHardwareMonitor | dernière version, **Remote Web Server** activé (ci-dessous) |

### Activer le Remote Web Server dans LibreHardwareMonitor

1. Ouvrir LibreHardwareMonitor **en tant qu'administrateur** (requis pour lire les capteurs)
2. **Options → Remote Web Server** → cocher **Run**
3. Le laisser tourner en tâche de fond

Il écoute sur `http://127.0.0.1:8085` (clés `lhm_host` / `lhm_port`).

---

## Installation

1. Télécharger l'archive de la dernière release sur
   [GitHub Releases](https://github.com/jordanatdown/monitoring-alert/releases) et la décompresser.
2. *(facultatif, installation neuve)* éditer `config.toml` du dossier décompressé.
3. Lancer **`install.bat`**. Clic droit › *Exécuter en tant qu'administrateur* la première
   fois **si une version 1.x est installée** (supprimer son service Windows l'exige) ;
   sinon un double-clic suffit.
4. Ouvrir **<http://127.0.0.1:8787/>**.

`install.bat` copie les fichiers dans `%LOCALAPPDATA%\Programs\MonitoringAlert\`, active le
démarrage automatique (clé `Run` de l'utilisateur) et démarre le processus. Il est
**idempotent** : le relancer ne casse rien.

### Migration depuis la version 1.x

C'est automatique, et **la base de données est conservée**. `install.bat` :

| Élément de la v1.x | Ce qui se passe |
|---|---|
| Service Windows `MonitoringAlert` | arrêté et supprimé (si ça échoue : l'installation s'arrête, relancez en administrateur) |
| Tâches planifiées de rapport (journalier / hebdo / mensuel) | supprimées |
| Enregistrement des notifications toast (AUMID) | supprimé |
| `apply-config.bat`, ancien `Register-Tasks.ps1` | supprimés |
| `config.toml` | clés obsolètes retirées (`reports_dir`, `*_report_*`, `retention_days`), vos autres valeurs conservées, ancienne version sauvegardée en `config.toml.v1.bak` |
| `temperatures.db` | **migrée sur place** au premier démarrage : une copie de sécurité `temperatures.db.v1.bak` est écrite, puis l'historique est agrégé par jour |
| Rapports `.md` du dossier `rapports\` | non touchés (supprimez-les si vous n'en voulez plus) |

Mise à jour depuis une 1.x avec l'ancien `update.bat` : ça fonctionne aussi (l'archive
contient un `Register-Tasks.ps1` de transition qui lance la migration). Les fois suivantes,
utilisez le nouveau `update.bat`.

Si le dossier `\MonitoringAlert\` vide reste visible dans le Planificateur de tâches, il est
inoffensif.

> **Mesures brutes** : par défaut seuls les **90 derniers jours** de mesures brutes sont gardés
> (`raw_retention_days`). Avant toute suppression, elles sont résumées par jour (médiane, min, p95,
> max) dans une table qui n'est **jamais purgée** : c'est elle qui permet de suivre la dérive sur toute
> la durée. Après la migration, la base peut donc rétrécir nettement.

### Repartir de zéro (en gardant la base)

Lancer **`uninstall.bat`** (administrateur) puis **`install.bat`**. `uninstall.bat` ne dépend pas
de la version installée (1.x ou 2.x) et conserve la base et `config.toml` sauf si vous répondez
« o » à sa dernière question.

### Mise à jour

**`update.bat`** (dans `%LOCALAPPDATA%\Programs\MonitoringAlert\`) télécharge la dernière release et
lance son installation ; `config.toml` et la base sont conservés.

---

## Le tableau de bord

**<http://127.0.0.1:8787/>**

- **En-tête** : état de la collecte (dernière mesure, LibreHardwareMonitor joignable ou non).
  Si LHM est arrêté, vous le voyez ici, sans être dérangé.
- **Charge de la machine** (Repos / Léger / Modéré / Intense) : une température n'a de sens qu'à
  charge équivalente ; on compare toujours « repos » avec « repos ». *Repos* est le plus sensible.
- **Tableau de tous les capteurs**, du plus fort écart au plus faible : température actuelle,
  référence, médiane 30 jours, **écart**, **pente** en °C/mois, mini-courbe, durée de suivi.
- **Graphique de dérive** du capteur choisi :
  - points : médiane de chaque jour, avec une bande claire du min au p95 ;
  - ligne : médiane glissante sur 30 jours (le signal à suivre) ;
  - tiretés : la **référence** (médiane des 30 premiers jours) ;
  - pointillés : droite de tendance ;
  - zone teintée entre la référence et la ligne, plus marquée quand l'écart grandit
    (chaude au-dessus, froide au-dessous) ;
  - **mois alternés** en fond : un été chaud ressemble à une dérive, la saisonnalité doit se voir.
  Glisser pour zoomer, double-clic pour revenir. Une *vue tableau* mensuelle est dépliable sous le graphique.

Tant qu'il y a moins de **60 jours** de suivi (et 40 jours avec mesures), aucune dérive n'est
affichée : seules les médianes quotidiennes le sont.

> **Limite connue** : il n'y a pas de capteur de température ambiante. Une saison chaude peut
> donc se voir comme une dérive de quelques degrés. Regardez la courbe sur plus d'un an et
> comparez les mêmes mois.

Le serveur n'écoute que sur **cette machine** et ne fait que lire. `web_host = "0.0.0.0"` l'ouvre
au réseau local, **sans authentification** : à ne faire que sur un réseau de confiance.

---

## Configuration

`%LOCALAPPDATA%\Programs\MonitoringAlert\config.toml` — toutes les clés sont facultatives.
Après modification : `monitoring-alert.exe restart`.

| Clé | Défaut | Rôle |
|---|---|---|
| `install_dir` | dossier du fichier | Dossier des exécutables |
| `db_path` | `temperatures.db` à côté du fichier | Base SQLite |
| `log_dir` | dossier de la base | Logs (`monitoring-alert.AAAA-MM-JJ.log`, rotation quotidienne) |
| `collect_interval_secs` | `300` | Intervalle de collecte (minimum 60) |
| `raw_retention_days` | `90` | Jours de mesures brutes conservées (minimum 7) |
| `lhm_host` / `lhm_port` | `127.0.0.1` / `8085` | Serveur HTTP de LibreHardwareMonitor |
| `web_host` / `web_port` | `127.0.0.1` / `8787` | Écoute du tableau de bord |
| `log_level` | `info` | `error` · `warn` · `info` · `debug` · `trace` |

---

## Ligne de commande

`monitoring-alert.exe` (console). `monitoring-daemon.exe` est la copie sans fenêtre lancée
automatiquement ; Task Manager affiche ce nom.

| Commande | Rôle |
|---|---|
| `install` | Installe / met à jour / migre depuis la 1.x (ce que fait `install.bat`) |
| `uninstall` | Arrête le processus, retire le démarrage automatique (ne supprime aucun fichier) |
| `start` · `stop` · `restart` | Gère le processus d'arrière-plan |
| `serve` | Collecte + tableau de bord au premier plan |
| `collect` | Une seule collecte (debug) |
| `check` | Vérifie config, base, LHM, tableau de bord, démarrage automatique, reliquats 1.x. Ne modifie rien |
| `db stats` · `db vacuum` | Chiffres de la base · compactage |
| `--db <chemin>` | (global) base à utiliser à la place de celle de `config.toml` |

---

## Comment la dérive est calculée

1. Toutes les 5 minutes, un *snapshot* : températures de tous les capteurs + charge CPU/GPU.
2. **Catégorie de charge** : charge effective = **max(CPU, GPU)** (un jeu à GPU 90 % / CPU 15 % est
   *intense*). `idle` < 15 % ≤ `light` < 40 % ≤ `moderate` < 75 % ≤ `heavy`.
3. **Agrégats quotidiens** par (jour, capteur, catégorie) : nombre de mesures, min, médiane, p95, max, moyenne.
   Un jour compte à partir de 6 mesures. Le jour en cours est exclu (incomplet).
4. **Référence** : médiane des médianes des 30 premiers jours. **Médiane glissante** : médiane des médianes
   des 30 derniers jours calendaires (≥ 10 jours dans la fenêtre).
5. **Écart** = médiane glissante actuelle − référence. **Pente** = droite des moindres carrés sur toutes les
   médianes quotidiennes, en °C/mois.

Rien ne déclenche d'alerte : ce sont des chiffres à regarder.

---

## Base de données

SQLite, mode WAL (le tableau de bord lit pendant que le collecteur écrit).

| Table | Contenu | Conservation |
|---|---|---|
| `snapshots` | `ts` (heure locale), `cpu_load`, `gpu_load`, `load_cat` | `raw_retention_days` |
| `readings` | une température par capteur et par snapshot | `raw_retention_days` |
| `daily_stats` | `day, hardware, sensor, load_cat, n, min, median, p95, max, avg` | **illimitée** |

Version du schéma : `PRAGMA user_version` (2). La purge supprime des jours entiers et agrège ce qu'elle
supprime dans la même transaction ; après une grosse purge le fichier est compacté (`VACUUM`).

---

## Supervision

- **`monitoring-alert.exe check`** : état complet, sans rien modifier.
- **En-tête du tableau de bord** : dernière mesure et erreur éventuelle.
- **Logs** : `monitoring-alert.AAAA-MM-JJ.log` dans `log_dir`. Le collecteur est relancé automatiquement
  s'il échoue, et un échec de collecte n'arrête pas le processus.
- Si le processus ne tourne pas (port déjà pris, base inaccessible), l'erreur est dans le log : le
  processus d'arrière-plan n'a pas de console.

---

## Développement

```
backend/    Rust : la bibliothèque, la CLI, le démon et le serveur web
frontend/   TypeScript (Vite + uPlot) : le tableau de bord
scripts/    install.bat, update.bat, uninstall.bat, config.toml, release.sh
```

### Architecture du backend (hexagonale simplifiée)

```
domain/       code pur, sans E/S : modèle, catégories de charge, calcul de dérive (trends)
application/  cas d'usage + ports (traits) : collecte, boucle, requêtes du tableau de bord, installation
adapters/     le technique : sqlite, lhm (HTTP), http (serveur web), config, logging, windows, pe, deploy
daemon.rs     racine de composition : branche les adapters sur les cas d'usage
```

Les dépendances ne vont que vers l'intérieur (adapters → application → domain) ; `tests/architecture.rs` le vérifie.

### Commandes

```bash
# Backend (depuis backend/)
cargo test
cargo clippy --all-targets --all-features -- -D warnings
cargo clippy --target x86_64-pc-windows-gnu --all-targets -- -D warnings   # code Windows, depuis Linux

# Frontend (depuis frontend/)
npm ci
npm run dev        # proxy /api vers 127.0.0.1:8787 (lancer `monitoring-alert serve` à côté)
npm run build      # produit frontend/dist, embarqué dans l'exécutable

# Exécutable complet
npm run build --prefix frontend && cargo build --release --manifest-path backend/Cargo.toml
```

Le frontend est embarqué dans l'exécutable par `backend/build.rs`. Sans build du frontend, `cargo test`
fonctionne (page de remplacement) ; un `cargo build --release` est refusé
(`MONITORING_ALERT_ALLOW_NO_FRONTEND=1` pour passer outre).

Pour essayer le tout sans Windows : `monitoring-alert serve` avec un faux serveur LHM (un script qui sert
un JSON `/data.json` sur `lhm_port`).

Conventions du projet (commits, branche, tests, documentation) : voir [CLAUDE.md](CLAUDE.md).
