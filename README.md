# monitoring-alert — Moniteur de température long terme

Service Windows de surveillance thermique pour détecter les dérives dues à la
poussière, une pâte thermique dégradée ou un ventilateur défaillant.

---

## Prérequis

| Logiciel | Version minimale | Notes |
|---|---|---|
| LibreHardwareMonitor | dernière version | WMI activé (voir ci-dessous) |

### Activer le support WMI dans LibreHardwareMonitor

1. Ouvrir LibreHardwareMonitor en tant qu'administrateur
2. Aller dans **Options → WMI Provider** → cocher **Enable WMI Provider**
3. Laisser LibreHardwareMonitor tourner en tâche de fond (ou configuré en service)

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
db_path     = "C:\\ProgramData\\MonitoringAlert\\temperatures.db"
```

### 3. Lancer `install.bat` en tant qu'administrateur

Le script effectue les étapes suivantes :

1. Copie `monitoring-alert.exe` dans `install_dir`
2. Crée `C:\ProgramData\MonitoringAlert\` et y dépose `config.toml`, `uninstall.bat`, `update.bat`
3. Enregistre l'AUMID `MonitoringAlert.TemperatureMonitor` dans le registre (nécessaire pour les notifications toast)
4. Enregistre et démarre le service Windows
5. Crée les tâches planifiées de rapport selon la configuration

Le dossier temporaire peut ensuite être supprimé.

### Après installation

```
C:\Program Files\MonitoringAlert\
    monitoring-alert.exe

C:\ProgramData\MonitoringAlert\
    config.toml          ← toute la configuration
    temperatures.db      ← base de données (créée au 1er démarrage)
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

**Fichier :** `C:\ProgramData\MonitoringAlert\config.toml`

C'est le seul fichier à modifier. Toutes les options sont regroupées ici.

```toml
# =============================================================
#  MonitoringAlert — configuration complète
# =============================================================

# Dossier d'installation de l'exécutable
install_dir = "C:\\Program Files\\MonitoringAlert"

# Chemin complet de la base de données SQLite
# → placer sur un lecteur sauvegardé si souhaité
db_path = "C:\\ProgramData\\MonitoringAlert\\temperatures.db"

# Intervalle de collecte du service (en secondes, minimum 60)
# Défaut : 300 (toutes les 5 minutes)
collect_interval_secs = 300

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
| `db_path` | chemin | `C:\ProgramData\MonitoringAlert\temperatures.db` | Chemin de la base SQLite |
| `collect_interval_secs` | entier ≥ 60 | `300` | Intervalle entre deux collectes (secondes) |
| `retention_days` | entier ≥ 180 | `365` | Durée de rétention des données (jours) |
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
| Paramètres de rapport | Lancer `update.bat` en tant qu'administrateur |

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

Ces tâches sont visibles dans le **Planificateur de tâches** sous
`Bibliothèque\MonitoringAlert`.

### Déclencher manuellement une notification

```powershell
monitoring-alert.exe notify --daily
monitoring-alert.exe notify --weekly
monitoring-alert.exe notify --monthly
```

---

## Mise à jour

Lancer `update.bat` (dans `C:\ProgramData\MonitoringAlert\`) **en tant qu'administrateur**.

Le script :
1. Arrête le service
2. Télécharge la dernière release depuis GitHub
3. Remplace l'exécutable
4. Redémarre le service
5. **Resynchronise les tâches planifiées** avec la configuration actuelle de `config.toml`
   (active, désactive ou met à jour les horaires selon les valeurs courantes)

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

# Utiliser une DB spécifique (override config.toml)
monitoring-alert.exe --db "D:\autre\temperatures.db" report

# Gestion du service
monitoring-alert.exe service install
monitoring-alert.exe service start
monitoring-alert.exe service stop
monitoring-alert.exe service uninstall
```

---

## Lecture du rapport

Le rapport compare les **moyennes à charge identique** sur 4 fenêtres :

| Fenêtre | Période courante | Période de référence |
|---|---|---|
| 24h | dernières 24h | 24h précédentes |
| 7j | 7 derniers jours | 7j précédents |
| 30j | 30 derniers jours | 30j précédents |
| 90j | 90 derniers jours | 90j précédents |

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

À chaque intervalle (par défaut 300 s), le service lit via WMI (LibreHardwareMonitor) :
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

Pour chaque fenêtre d'analyse (24 h, 7 j, 30 j, 90 j), on calcule :

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
(défaut : 365 jours, minimum : 180 jours). Le défaut d'un an permet de voir les
effets saisonniers (été/hiver) et l'évolution long terme. La valeur minimale de
180 jours est imposée car l'algorithme compare une fenêtre courante de 90 jours à
la fenêtre précédente de 90 jours.

---

## Causes possibles d'une dérive thermique

- **Poussière** : nettoyer les filtres et radiateurs à l'air comprimé
- **Pâte thermique** : renouveler si le système a plus de 2–3 ans
- **Ventilateur défaillant** : vérifier le bruit et la vitesse de rotation

---

## Supervision

Les logs du service sont visibles dans l'Observateur d'événements Windows :
**Applications et services → MonitoringAlert**

Ou via PowerShell :
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
