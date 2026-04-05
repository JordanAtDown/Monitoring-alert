# monitoring-alert — Moniteur de température long terme

Service Windows de surveillance thermique pour détecter les dérives dues à la
poussière, une pâte thermique dégradée ou un ventilateur défaillant.

---

## Prérequis

| Logiciel | Version minimale | Notes |
|---|---|---|
| Rust | 1.75+ | `rustup target add x86_64-pc-windows-msvc` |
| Visual Studio Build Tools | 2022 | Toolchain MSVC |
| LibreHardwareMonitor | dernière version | WMI activé (voir ci-dessous) |

### Activer le support WMI dans LibreHardwareMonitor

1. Ouvrir LibreHardwareMonitor en tant qu'administrateur
2. Aller dans **Options → WMI Provider** → cocher **Enable WMI Provider**
3. Laisser LibreHardwareMonitor tourner en tâche de fond (ou configuré en service)

---

## Compilation

```powershell
# Mode debug
cargo build

# Mode release (production)
cargo build --release --target x86_64-pc-windows-msvc
```

L'exécutable est dans `target\x86_64-pc-windows-msvc\release\monitoring-alert.exe`.

---

## Installation du service

> ⚠ Les commandes `service install`, `uninstall`, `start`, `stop` nécessitent
> une invite de commandes **administrateur**.

```powershell
# 1. Copier l'exécutable dans un emplacement permanent
mkdir "C:\Program Files\MonitoringAlert"
copy target\x86_64-pc-windows-msvc\release\monitoring-alert.exe "C:\Program Files\MonitoringAlert\"

# 2. Installer et démarrer le service Windows
cd "C:\Program Files\MonitoringAlert"
.\monitoring-alert.exe service install
.\monitoring-alert.exe service start
```

Le service :
- Tourne sous le nom **MonitoringAlert**
- Démarre automatiquement au démarrage de Windows
- Stocke la base de données dans `C:\ProgramData\MonitoringAlert\temperatures.db`
- Se redémarre automatiquement après un crash (3 tentatives, délai 5 s)

### Désinstallation

```powershell
# Invite admin requise
cd "C:\Program Files\MonitoringAlert"
.\monitoring-alert.exe service stop
.\monitoring-alert.exe service uninstall

# Optionnel : supprimer les fichiers
Remove-Item -Recurse "C:\Program Files\MonitoringAlert"
Remove-Item -Recurse "C:\ProgramData\MonitoringAlert"   # ⚠ supprime aussi la DB
```

### Mise à jour

```powershell
# 1. Arrêter le service
"C:\Program Files\MonitoringAlert\monitoring-alert.exe" service stop

# 2. Recompiler
cargo build --release --target x86_64-pc-windows-msvc

# 3. Remplacer l'exe
copy /Y target\x86_64-pc-windows-msvc\release\monitoring-alert.exe "C:\Program Files\MonitoringAlert\"

# 4. Redémarrer le service
"C:\Program Files\MonitoringAlert\monitoring-alert.exe" service start
```

La DB dans `C:\ProgramData\MonitoringAlert\` est conservée intacte entre les mises à jour.

---

## Utilisation en ligne de commande

```powershell
# Collecte unique (test / debug)
monitoring-alert.exe collect

# Boucle de collecte toutes les 5 minutes (même logique que le service)
monitoring-alert.exe watch --interval 300

# Rapport sur stdout
monitoring-alert.exe report

# Rapport dans un fichier
monitoring-alert.exe report -o rapport.txt
```

---

## Structure de la base de données

```
C:\ProgramData\MonitoringAlert\temperatures.db   (service)
.\temperatures.db                                 (CLI)
```

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

## Supervision / Journalisation

Les logs du service sont visibles dans l'Observateur d'événements Windows :
**Applications et services → MonitoringAlert**

Ou via PowerShell :
```powershell
Get-EventLog -LogName Application -Source MonitoringAlert -Newest 50
```

---

## Développement

Voir [CLAUDE.md](../CLAUDE.md) pour les conventions de contribution.
