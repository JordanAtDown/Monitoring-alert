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

```toml
# Dossier où sera installé l'exécutable
install_dir = "C:\\Program Files\\MonitoringAlert"

# Chemin de la base de données de températures
# Placer sur un lecteur sauvegardé si souhaité
db_path = "C:\\ProgramData\\MonitoringAlert\\temperatures.db"
```

### 3. Lancer `install.bat` en tant qu'administrateur

Le script :
1. Copie `monitoring-alert.exe` dans `install_dir`
2. Crée `C:\ProgramData\MonitoringAlert\`
3. Y dépose `config.toml`, `uninstall.bat` et `update.bat`
4. Enregistre et démarre le service Windows

Le dossier temporaire peut ensuite être supprimé.

### Après installation

```
C:\Program Files\MonitoringAlert\      ← exécutable uniquement
C:\ProgramData\MonitoringAlert\
    config.toml                        ← configuration (install_dir, db_path)
    temperatures.db                    ← base de données
    uninstall.bat                      ← désinstallation
    update.bat                         ← mise à jour
```

Le service :
- Tourne sous le nom **MonitoringAlert**
- Démarre automatiquement au démarrage de Windows
- Se redémarre automatiquement après un crash (3 tentatives, délai 5 s)
- Est visible dans `services.msc` ou via `sc query MonitoringAlert`

---

## Mise à jour

Lancer `update.bat` (dans `C:\ProgramData\MonitoringAlert\`) **en tant qu'administrateur**.

Le script :
1. Arrête le service
2. Télécharge la dernière release depuis GitHub
3. Remplace l'exécutable
4. Redémarre le service

`config.toml` et `temperatures.db` sont conservés intacts.

---

## Désinstallation

Lancer `uninstall.bat` (dans `C:\ProgramData\MonitoringAlert\`) **en tant qu'administrateur**.

Le script :
1. Arrête et supprime le service
2. Supprime le dossier d'installation (exe)
3. Demande confirmation avant de supprimer `C:\ProgramData\MonitoringAlert\` (DB + config)

---

## Configuration

`C:\ProgramData\MonitoringAlert\config.toml` est le seul fichier à modifier.

| Clé | Description | Défaut |
|---|---|---|
| `install_dir` | Dossier de l'exécutable | `C:\Program Files\MonitoringAlert` |
| `db_path` | Chemin complet de la base SQLite | `C:\ProgramData\MonitoringAlert\temperatures.db` |

Pour appliquer un nouveau `db_path` : modifier le fichier et redémarrer le service
(`sc stop MonitoringAlert && sc start MonitoringAlert`).

---

## Utilisation en ligne de commande

```powershell
# Collecte unique (test / debug)
monitoring-alert.exe collect

# Boucle de collecte (même logique que le service)
monitoring-alert.exe watch --interval 300

# Rapport sur stdout
monitoring-alert.exe report

# Rapport dans un fichier
monitoring-alert.exe report -o rapport.txt

# Utiliser une DB spécifique (override config.toml)
monitoring-alert.exe --db "D:\autre\temperatures.db" report
```

---

## Structure de la base de données

Deux tables SQLite :

- **`snapshots`** — un enregistrement par mesure (timestamp, charge CPU/GPU, catégorie)
- **`readings`** — une ligne par capteur par snapshot (matériel, capteur, valeur °C)

---

## Lecture du rapport

Le rapport compare les **moyennes à charge identique** sur 4 fenêtres :

| Fenêtre | Période courante | Période de référence |
|---|---|---|
| 24h | dernières 24h | 24h précédentes |
| 7j | 7 derniers jours | 7j précédents |
| 30j | 30 derniers jours | 30j précédents |
| 90j | 90 derniers jours | 90j précédents |

Seuils d'alerte :

| Delta | Statut |
|---|---|
| ≥ 10 °C | 🔴 CRITIQUE ← nettoyer ! |
| ≥ 5 °C | ⚠ ATTENTION |
| ≥ 2 °C | ↑ légère hausse |
| ≤ −2 °C | ↓ amélioration |
| autre | ✓ stable |

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

Voir [CLAUDE.md](../CLAUDE.md) pour les conventions de contribution.

```powershell
# Prérequis : Rust + MSVC toolchain
rustup target add x86_64-pc-windows-msvc

# Build release
cargo build --release --target x86_64-pc-windows-msvc
```
