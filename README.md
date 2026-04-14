# monitoring-alert — Moniteur de température long terme

Service Windows de surveillance thermique pour détecter les dérives dues à la
poussière, une pâte thermique dégradée ou un ventilateur défaillant.

---

## Prérequis

| Logiciel | Version minimale | Notes |
|---|---|---|
| LibreHardwareMonitor | dernière version | Remote Web Server activé (voir ci-dessous) |

### Activer le Remote Web Server dans LibreHardwareMonitor

1. Ouvrir LibreHardwareMonitor **en tant qu'administrateur** (requis pour lire les capteurs)
2. Aller dans **Options → Remote Web Server** → cocher **Run**
3. Laisser LibreHardwareMonitor tourner en tâche de fond

Le serveur écoute par défaut sur `http://127.0.0.1:8085`. L'adresse et le port sont
configurables dans `config.toml` via les clés `lhm_host` et `lhm_port`.

---

## Installation

### 1. Télécharger la dernière release

Télécharger l'archive depuis la page [Releases](https://github.com/jordanatdown/monitoring-alert/releases)
et extraire le contenu dans un dossier temporaire :

```
monitoring-alert-v0.x.x\
├── monitoring-alert.exe
├── config.toml       ← à éditer avant l'installation
├── install.bat
├── uninstall.bat
└── update.bat
```

### 2. Éditer `config.toml`

Configurer au minimum `install_dir` et `db_path` (voir la section
[Configuration](#configuration) pour la liste complète des options).

```toml
install_dir = "C:\\Program Files\\MonitoringAlert"
db_path     = "C:\\Users\\<VotreNom>\\AppData\\Local\\Programs\\MonitoringAlert\\temperatures.db"
```

### 3. Lancer `install.bat` en tant qu'administrateur

Le script effectue les étapes suivantes :

1. Copie `monitoring-alert.exe` dans `install_dir`
2. Crée `%LOCALAPPDATA%\Programs\MonitoringAlert\` et y dépose `config.toml`, `uninstall.bat`, `update.bat`
3. Enregistre l'AUMID `MonitoringAlert.TemperatureMonitor` dans le registre (nécessaire pour les notifications toast)
4. Enregistre et démarre le service Windows
5. Crée les tâches planifiées de rapport selon la configuration

Le dossier temporaire peut ensuite être supprimé.

### Après installation

```
C:\Program Files\MonitoringAlert\
    monitoring-alert.exe

%LOCALAPPDATA%\Programs\MonitoringAlert\   (ex. C:\Users\Jordan\AppData\Local\Programs\MonitoringAlert\)
    config.toml                ← toute la configuration
    temperatures.db            ← base de données (créée au 1er démarrage)
    monitoring-alert-YYYY-MM-DD.log  ← logs du service (rotation quotidienne)
    uninstall.bat
    update.bat

Planificateur de tâches\MonitoringAlert\
    RapportJournalier    ← si daily_report_enabled = true
    RapportHebdomadaire  ← si weekly_report_enabled = true
    RapportMensuel       ← si monthly_report_enabled = true
```

Le service :
- Tourne sous le nom **MonitoringAlert**
- Démarre automatiquement au démarrage de Windows
- Se redémarre automatiquement après un crash (3 tentatives, délai 5 s)
- Est visible dans `services.msc` ou via `sc query MonitoringAlert`

---

## Configuration

**Fichier :** `%LOCALAPPDATA%\Programs\MonitoringAlert\config.toml`

C'est le seul fichier à modifier. Toutes les options sont regroupées ici.

```toml
# =============================================================
#  MonitoringAlert — configuration complète
# =============================================================

# Dossier d'installation de l'exécutable
install_dir = "C:\\Program Files\\MonitoringAlert"

# Chemin complet de la base de données SQLite
# → placer sur un lecteur sauvegardé si souhaité
db_path = "C:\\Users\\Jordan\\AppData\\Local\\Programs\\MonitoringAlert\\temperatures.db"

# Dossier des fichiers de log (défaut : même répertoire que db_path)
# log_dir = "C:\\Users\\Jordan\\AppData\\Local\\Programs\\MonitoringAlert\\logs"

# Intervalle de collecte du service (en secondes, minimum 60)
# Défaut : 300 (toutes les 5 minutes)
collect_interval_secs = 300

# Adresse IP du serveur HTTP de LibreHardwareMonitor
lhm_host = "127.0.0.1"

# Port du serveur HTTP de LibreHardwareMonitor
lhm_port = 8085

# =============================================================
#  Rapports automatiques via notifications Windows
# =============================================================

# Rapport journalier
daily_report_enabled = true
daily_report_time    = "08:00"    # HH:MM (format 24h)

# Rapport hebdomadaire
weekly_report_enabled = false
weekly_report_day     = "MON"     # MON TUE WED THU FRI SAT SUN
weekly_report_time    = "08:00"

# Rapport mensuel
monthly_report_enabled = false
monthly_report_day     = 1        # 1–28
monthly_report_time    = "08:00"
```

### Référence des clés

| Clé | Type | Défaut | Description |
|---|---|---|---|
| `install_dir` | chemin | `C:\Program Files\MonitoringAlert` | Dossier de l'exécutable |
| `db_path` | chemin | même dossier que `config.toml` | Chemin de la base SQLite |
| `log_dir` | chemin | même dossier que `db_path` | Dossier des fichiers de log |
| `lhm_host` | chaîne | `"127.0.0.1"` | Adresse du serveur HTTP de LibreHardwareMonitor |
| `lhm_port` | entier | `8085` | Port du serveur HTTP de LibreHardwareMonitor |
| `collect_interval_secs` | entier ≥ 60 | `300` | Intervalle entre deux collectes (secondes) |
| `retention_days` | entier ≥ 360 | `365` | Durée de rétention des données (jours) |
| `log_level` | `"error"` … `"trace"` | `"info"` | Niveau de log (utiliser `"debug"` pour diagnostiquer) |
| `daily_report_enabled` | booléen | `true` | Activer le rapport journalier |
| `daily_report_time` | `"HH:MM"` | `"08:00"` | Heure d'envoi du rapport journalier |
| `weekly_report_enabled` | booléen | `false` | Activer le rapport hebdomadaire |
| `weekly_report_day` | `"MON"`…`"SUN"` | `"MON"` | Jour d'envoi du rapport hebdomadaire |
| `weekly_report_time` | `"HH:MM"` | `"08:00"` | Heure d'envoi du rapport hebdomadaire |
| `monthly_report_enabled` | booléen | `false` | Activer le rapport mensuel |
| `monthly_report_day` | entier 1–28 | `1` | Jour du mois d'envoi du rapport mensuel |
| `monthly_report_time` | `"HH:MM"` | `"08:00"` | Heure d'envoi du rapport mensuel |

### Appliquer les changements

| Modification | Action |
|---|---|
| `db_path` | Redémarrer le service : `sc stop MonitoringAlert && sc start MonitoringAlert` |
| `collect_interval_secs` | Redémarrer le service |
| `retention_days` | Redémarrer le service |
| `log_level` | Redémarrer le service |
| Paramètres de rapport | Lancer `apply-config.bat` en tant qu'administrateur |
| Tous les autres | Lancer `apply-config.bat` en tant qu'administrateur |

`apply-config.bat` relit `config.toml`, redémarre le service et resynchronise
les tâches planifiées — sans télécharger de nouvelle version.

---

## Notifications de rapport

Les rapports sont envoyés sous forme de **notifications toast Windows** à l'heure
configurée. Ils comparent les températures des 30 derniers jours par rapport aux
30 jours précédents, à charge identique (idle et heavy séparément).

**Exemple de notification :**
```
MonitoringAlert — Rapport Journalier
⚠ 2 alertes — GPU Temperature: +7.0°C sur 30j
```

Ou si tout va bien :
```
MonitoringAlert — Rapport Journalier
✓ Toutes les températures stables
```

### Fonctionnement technique

Le service Windows tourne en session SYSTEM (Session 0) et ne peut pas afficher
de notifications à l'écran. `install.bat` crée donc des **tâches planifiées Windows**
qui s'exécutent dans la session de l'utilisateur connecté et appellent :

```powershell
monitoring-alert.exe notify --daily    # rapport journalier
monitoring-alert.exe notify --weekly   # rapport hebdomadaire
monitoring-alert.exe notify --monthly  # rapport mensuel
```

Les tâches sont créées avec **`StartWhenAvailable`** : si le PC était éteint à
l'heure prévue, la notification est envoyée dès la prochaine ouverture de session.

Ces tâches sont visibles dans le **Planificateur de tâches** sous
`Bibliothèque\MonitoringAlert`.

### Déclencher manuellement une notification

```powershell
monitoring-alert.exe notify --daily
monitoring-alert.exe notify --weekly
monitoring-alert.exe notify --monthly
```

### Tester les toasts sans données (`notify-dry-run`)

Envoie des messages fictifs qui répliquent chaque variante possible du toast,
sans accéder à la base de données. Utile pour vérifier que l'AUMID est bien
enregistré et que les toasts s'affichent correctement.

```powershell
# Tous les scénarios en séquence (pause de 4 s entre chaque)
monitoring-alert.exe notify-dry-run

# Scénario précis
monitoring-alert.exe notify-dry-run --scenario stable
monitoring-alert.exe notify-dry-run --scenario attention
monitoring-alert.exe notify-dry-run --scenario critique
monitoring-alert.exe notify-dry-run --scenario multi

# Avec titre hebdomadaire ou mensuel
monitoring-alert.exe notify-dry-run --scenario all --period weekly
```

| Scénario | Corps du toast |
|---|---|
| `stable` | `✓ Toutes les températures stables` |
| `attention` | `⚠ 1 alerte — CPU Package: +6.0°C sur 30j` |
| `critique` | `⚠ 1 alerte — GPU Junction Temperature: +12.0°C sur 30j` |
| `multi` | `⚠ 3 alertes — GPU Junction Temperature: +12.0°C sur 30j` |

Le titre est suffixé `[TEST: <scénario>]` pour distinguer les toasts de test
des vrais rapports dans le centre de notifications.

### Notifications pendant les sessions de jeu

Les notifications Windows peuvent apparaître par-dessus un jeu en mode
fenêtré ou borderless. Ce comportement est contrôlé par Windows, pas par
l'application.

Pour désactiver les interruptions pendant les parties, activez
**Focus Assist → Priorité uniquement** ou **Alarmes uniquement** dans
`Paramètres → Système → Notifications` (Windows 10) ou
`Paramètres → Système → Ne pas déranger` (Windows 11).

Sur Windows 11, l'option **"Activer Ne pas déranger automatiquement → Lors
de l'utilisation d'une application en plein écran"** bloque les toasts
uniquement pendant les jeux en plein écran exclusif.

---

## Mise à jour

Lancer `update.bat` (dans `%LOCALAPPDATA%\Programs\MonitoringAlert\`) **en tant qu'administrateur**.

Le script :
1. Arrête le service
2. Télécharge l'archive zip de la dernière release depuis GitHub
3. Remplace l'exécutable **et** les scripts (`Register-Tasks.ps1`, `apply-config.bat`, `uninstall.bat`)
4. Redémarre le service
5. **Resynchronise les tâches planifiées** avec la configuration actuelle de `config.toml`

`config.toml` et `temperatures.db` sont conservés intacts.

---

## Désinstallation

Lancer `uninstall.bat` (dans `C:\ProgramData\MonitoringAlert\`) **en tant qu'administrateur**.

Le script :
1. Arrête et supprime le service Windows
2. Supprime le dossier d'installation (exe)
3. Supprime les tâches planifiées de rapport
4. Supprime l'AUMID du registre
5. Demande confirmation avant de supprimer `C:\ProgramData\MonitoringAlert\` (DB + config)

---

## Utilisation en ligne de commande

```powershell
# Collecte unique (test / debug)
monitoring-alert.exe collect

# Boucle de collecte (même logique que le service)
monitoring-alert.exe watch --interval 300

# Rapport détaillé sur stdout
monitoring-alert.exe report

# Rapport dans un fichier
monitoring-alert.exe report -o rapport.txt

# Envoyer une notification toast manuellement
monitoring-alert.exe notify --daily
monitoring-alert.exe notify --weekly
monitoring-alert.exe notify --monthly

# Tester les toasts sans données (dry-run)
monitoring-alert.exe notify-dry-run                          # envoie les 4 scénarios en séquence
monitoring-alert.exe notify-dry-run --scenario stable        # ✓ Toutes les températures stables
monitoring-alert.exe notify-dry-run --scenario attention     # ⚠ 1 alerte — seuil ATTENTION
monitoring-alert.exe notify-dry-run --scenario critique      # ⚠ 1 alerte — seuil CRITIQUE
monitoring-alert.exe notify-dry-run --scenario multi         # ⚠ 3 alertes
monitoring-alert.exe notify-dry-run --scenario all --period weekly  # titre hebdomadaire

# Utiliser une DB spécifique (override config.toml)
monitoring-alert.exe --db "D:\autre\temperatures.db" report

# Gestion du service
monitoring-alert.exe service install
monitoring-alert.exe service start
monitoring-alert.exe service stop
monitoring-alert.exe service uninstall
monitoring-alert.exe service status        # état du service (installé, actif, PID)

# Vérification pré-démarrage
monitoring-alert.exe check                 # config, DB, service, LHM, AUMID

# Maintenance de la base de données
monitoring-alert.exe db stats              # taille, nb snapshots, dates
monitoring-alert.exe db vacuum             # compacte et libère l'espace disque
```

---

## Lecture du rapport

Le rapport compare les **moyennes à charge identique** sur 5 fenêtres :

| Fenêtre | Période courante | Période de référence | Signal typique |
|---|---|---|---|
| 24h | dernières 24h | 24h précédentes | Pic ponctuel, ventilateur bloqué |
| 7j | 7 derniers jours | 7j précédents | Début d'encrassement |
| 30j | 30 derniers jours | 30j précédents | Poussière, pâte qui sèche |
| 90j | 90 derniers jours | 90j précédents | Tendance saisonnière courte |
| 180j | 180 derniers jours | 180j précédents | Dérive annuelle, vieillissement matériel |

Les catégories de charge sont analysées séparément :

| Catégorie | Charge effective |
|---|---|
| `idle` | 0–14 % |
| `light` | 15–39 % |
| `moderate` | 40–74 % |
| `heavy` | 75–100 % |

Seuils d'alerte :

| Delta | Statut |
|---|---|
| ≥ 10 °C | 🔴 CRITIQUE ← nettoyer ! |
| ≥ 5 °C | ⚠ ATTENTION |
| ≥ 2 °C | ↑ légère hausse |
| ≤ −2 °C | ↓ amélioration |
| autre | ✓ stable |

---

## Algorithme de détection de dérive thermique

### 1. Collecte des données

À chaque intervalle (par défaut 300 s), le service interroge l'API HTTP de LibreHardwareMonitor (`http://lhm_host:lhm_port/data.json`) pour lire :
- toutes les sondes de température disponibles (CPU, GPU, chipset, stockage…)
- la charge CPU (%) et la charge GPU (%)

Un **snapshot** est enregistré en base avec l'horodatage, les charges CPU/GPU et la
catégorie de charge calculée. Chaque lecture de sonde constitue une ligne **reading**
liée au snapshot.

### 2. Catégorisation de la charge — charge effective

La catégorie est calculée à partir de la **charge effective** :

```
charge_effective = max(charge_cpu, charge_gpu)
```

Pourquoi `max` et non `cpu` seul ? Un scénario de jeu typique présente GPU 90 % /
CPU 15 %. Si l'on utilisait uniquement la charge CPU, le snapshot serait classé
`light` et les températures GPU enregistrées pendant cette session seraient comparées
à des périodes de navigation web au lieu d'autres sessions de jeu — la comparaison
serait sans sens.

En prenant le maximum, les sessions à fort usage GPU sont correctement classées
`heavy` et comparées exclusivement à d'autres sessions `heavy`.

| Catégorie | Charge effective |
|---|---|
| `idle` | 0–14 % |
| `light` | 15–39 % |
| `moderate` | 40–74 % |
| `heavy` | 75–100 % |

### 3. Agrégation par fenêtre temporelle

Pour chaque fenêtre d'analyse (24 h, 7 j, 30 j, 90 j, 180 j), on calcule :

```
moyenne_courante  = AVG(temp)  WHERE timestamp ∈ [now - W,     now[
                                 AND load_cat   = <catégorie>
                                 AND hardware   = <matériel>
                                 AND sensor     = <sonde>

moyenne_reference = AVG(temp)  WHERE timestamp ∈ [now - 2W,    now - W[
                                 AND load_cat   = <catégorie>
                                 AND hardware   = <matériel>
                                 AND sensor     = <sonde>
```

Si l'une des deux fenêtres ne contient aucune donnée pour un couple
(matériel, sonde, catégorie), la comparaison est ignorée pour ce couple.

### 4. Calcul du delta et seuils d'alerte

```
delta = moyenne_courante − moyenne_reference
```

| Delta | Statut | Interprétation |
|---|---|---|
| ≥ 10 °C | 🔴 CRITIQUE | Dérive majeure — nettoyer d'urgence |
| ≥ 5 °C | ⚠ ATTENTION | Dérive notable à surveiller |
| ≥ 2 °C | ↑ légère hausse | Tendance ascendante |
| ≤ −2 °C | ↓ amélioration | Refroidissement ou charge réduite |
| autre | ✓ stable | Aucune dérive significative |

Une **alerte** est déclenchée si `delta ≥ 5 °C` (ATTENTION ou CRITIQUE).
Le rapport liste les alertes par delta décroissant et inclut toutes les fenêtres
d'analyse.

---

## Structure de la base de données

Deux tables SQLite :

- **`snapshots`** — un enregistrement par mesure (timestamp, charge CPU/GPU, catégorie de charge)
- **`readings`** — une ligne par capteur par snapshot (matériel, nom du capteur, valeur °C)

### Croissance et rétention

Estimation de la taille avec 10 sondes et une collecte toutes les 5 min :

| Période | Taille estimée |
|---|---|
| 1 mois | ~7 MB |
| 6 mois | ~42 MB |
| 1 an | ~90 MB |

Le service purge automatiquement les données anciennes **au démarrage puis toutes les
24 h**. La durée de rétention est contrôlée par `retention_days` dans `config.toml`
(défaut : 365 jours, minimum : 360 jours). Le défaut d'un an permet d'activer
immédiatement la fenêtre saisonnière 180j. La valeur minimale de 360 jours est
imposée car l'algorithme compare une fenêtre courante de 180 jours à la fenêtre
précédente de 180 jours.

---

## Causes possibles d'une dérive thermique

- **Poussière** : nettoyer les filtres et radiateurs à l'air comprimé
- **Pâte thermique** : renouveler si le système a plus de 2–3 ans
- **Ventilateur défaillant** : vérifier le bruit et la vitesse de rotation

---

## Supervision

### Vérification pré-démarrage (`check`)

La commande `check` effectue un diagnostic complet de l'installation sans avoir
besoin de données existantes :

```powershell
monitoring-alert.exe check
```

Exemple de sortie :

```
Vérifications pré-démarrage
════════════════════════════════════════════════════════════════

  ✓  Configuration chargée
     DB         : C:\ProgramData\MonitoringAlert\temperatures.db
     Intervalle : 300 s  —  Rétention : 365 j  —  Log : info

  ✓  Base de données accessible  (0.0 MB — 0 snapshot(s))

  ✓  Service installé et actif

  ✓  LibreHardwareMonitor HTTP accessible (127.0.0.1:8085)  (12 sonde(s) de température)

  ✓  AUMID enregistré — notifications toast disponibles
     → Testez avec : monitoring-alert.exe notify-dry-run

════════════════════════════════════════════════════════════════
  Tout est opérationnel. ✓
```

| Vérification | Ce qui est testé | Action si ✗ |
|---|---|---|
| Configuration | Valeurs chargées depuis `config.toml` | Vérifier le fichier de config |
| Base de données | Ouverture + accès en écriture | Vérifier que le répertoire existe |
| Service | Installé + en cours d'exécution | `service install` puis `service start` |
| LHM HTTP | `GET http://lhm_host:lhm_port/data.json` | Lancer LHM admin + activer Options › Remote Web Server › Run |
| AUMID | Clé registre pour les toasts | Relancer `install.bat` admin |

### État du service

```powershell
monitoring-alert.exe service status
# → Service MonitoringAlert — ✓  En cours d'exécution  (PID 1234)
```

### Fichier de log

Le service écrit ses logs dans le répertoire `log_dir` (par défaut, même répertoire
que la base de données) :

```
%LOCALAPPDATA%\Programs\MonitoringAlert\monitoring-alert-YYYY-MM-DD.log
```

Le fichier est renouvelé chaque jour (rotation quotidienne). Pour écrire dans un
dossier différent, définir `log_dir` dans `config.toml`.

Le niveau de log est configurable dans `config.toml` (redémarrage du service requis) :

```toml
log_level = "info"   # production normale
log_level = "debug"  # diagnostiquer collecte, capteurs, purge…
log_level = "trace"  # très verbeux
```

Messages à surveiller :

| Message | Signification |
|---|---|
| `No temperature sensors detected` | LibreHardwareMonitor n'est pas lancé ou Remote Web Server inactif |
| `No temperature readings for N consecutive collections` | LHM absent depuis ~1h |
| `LHM unreachable at host:port` | LHM non lancé ou mauvais `lhm_host`/`lhm_port` dans config |
| `Collection error` | Erreur HTTP — vérifier LHM et que Remote Web Server est actif |
| `Purged N snapshot(s)` | Purge automatique exécutée |

### Observateur d'événements Windows

Les erreurs fatales du service (démarrage/arrêt anormal) apparaissent dans :
**Observateur d'événements → Applications et services → MonitoringAlert**

```powershell
Get-EventLog -LogName Application -Source MonitoringAlert -Newest 50
```

---

## Développement / Compilation

Voir [CLAUDE.md](CLAUDE.md) pour les conventions de contribution.

```powershell
# Prérequis : Rust + MSVC toolchain
rustup target add x86_64-pc-windows-msvc

# Build release
cargo build --release --target x86_64-pc-windows-msvc
```

Pour créer une nouvelle release :

```bash
./scripts/release.sh 0.2.0
```

Le script met à jour `Cargo.toml` et `CHANGELOG.md`, exécute les tests, crée
le tag annoté et pousse vers `origin`. GitHub Actions se charge ensuite de
compiler le binaire et de publier la release.
