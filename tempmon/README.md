# TempMon — Moniteur de température long terme

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

L'exécutable est dans `target\x86_64-pc-windows-msvc\release\tempmon.exe`.

---

## Installation du service

> ⚠ Les commandes `service install`, `uninstall`, `start`, `stop` nécessitent
> une invite de commandes **administrateur**.

```powershell
# 1. Copier l'exécutable dans un emplacement permanent, par exemple :
copy target\x86_64-pc-windows-msvc\release\tempmon.exe C:\Program Files\TempMon\

# 2. Installer le service Windows
C:\Program Files\TempMon\tempmon.exe service install

# 3. Démarrer le service
C:\Program Files\TempMon\tempmon.exe service start
```

Le service :
- Tourne sous le nom **TempMon**
- Démarre automatiquement au démarrage de Windows
- Stocke la base de données dans `C:\ProgramData\TempMon\temperatures.db`
- Se redémarre automatiquement après un crash (3 tentatives, délai 5 s)

### Désinstallation

```powershell
C:\Program Files\TempMon\tempmon.exe service stop
C:\Program Files\TempMon\tempmon.exe service uninstall
```

---

## Utilisation en ligne de commande

```powershell
# Collecte unique (test / debug)
tempmon.exe collect

# Boucle de collecte toutes les 5 minutes (même logique que le service)
tempmon.exe watch --interval 300

# Rapport sur stdout
tempmon.exe report

# Rapport dans un fichier
tempmon.exe report -o rapport.txt
```

---

## Structure de la base de données

```
C:\ProgramData\TempMon\temperatures.db   (service)
.\temperatures.db                         (CLI)
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
**Applications et services → TempMon**

Ou via PowerShell :
```powershell
Get-EventLog -LogName Application -Source TempMon -Newest 50
```

---

## Développement

Voir [CLAUDE.md](../CLAUDE.md) pour les conventions de contribution.
