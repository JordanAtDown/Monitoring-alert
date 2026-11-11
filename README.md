# Monitoring — Suivi long terme de la dérive des températures

*(dépôt GitHub : `monitoring-alert`)*

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

1. Télécharger **`monitoring_X.Y.Z.exe`** sur
   [GitHub Releases](https://github.com/jordanatdown/monitoring-alert/releases) et le lancer.
   Windows SmartScreen peut afficher « protégé votre ordinateur » (l'installateur n'est pas signé) :
   *Informations complémentaires › Exécuter quand même*.
2. L'assistant propose un dossier (par défaut `%LOCALAPPDATA%\Programs\Monitoring`, **aucun droit
   administrateur**) puis la **base de données** : s'il en trouve une dans l'installation actuelle, il la
   préremplit avec son contenu (« 4 300 relevés bruts, depuis le … ») ; gardez-la pour conserver tout votre
   historique, ou choisissez-en une autre.
3. À la fin, **Ouvrir le tableau de bord** : <http://127.0.0.1:8787/>. Un raccourci est aussi dans le menu Démarrer.

L'installateur copie le programme, puis lance `monitoring.exe install --in-place`, qui active le
démarrage automatique (clé `Run` de l'utilisateur) et démarre le processus. C'est **idempotent** :
relancer l'installateur met à jour sans rien casser, et `config.toml` n'est jamais écrasé.

Désinstaller : *Paramètres › Applications* (ou le menu Démarrer). Il propose de supprimer aussi le dossier de
données (non par défaut) ; une base placée dans un autre dossier n'est **jamais** supprimée.

### Mettre à jour

Une mise à jour est simplement **le nouvel installateur lancé par-dessus l'ancien** : il remplace le programme, le
redémarre, et conserve votre configuration et vos données (`/VERYSILENT` pour le faire sans fenêtre).

Quand une nouvelle version existe, le tableau de bord affiche en haut à droite un bouton **Mise à jour disponible**. Il ouvre
trois étapes : télécharger `monitoring_X.Y.Z.exe`, le lancer, suivre l'assistant. **Rien n'est téléchargé ni installé tout seul.**
Pour cela le programme demande une fois par jour à GitHub quelle est la dernière version (une requête anonyme, sans aucune
information sur votre machine) ; `check_updates = false` désactive cette recherche.

### Migration depuis la version 1.x

C'est automatique, et **la base de données est conservée**.
Si un service Windows 1.x est présent, l'installateur propose de se relancer en administrateur (c'est le seul
cas où il en faut : supprimer ce service l'exige). Il :

| Élément de la v1.x | Ce qui se passe |
|---|---|
| Service Windows `MonitoringAlert` | arrêté et supprimé (si ça échoue : l'installation s'arrête, relancez en administrateur) |
| Tâches planifiées de rapport (journalier / hebdo / mensuel) | supprimées |
| Enregistrement des notifications toast (AUMID) | supprimé |
| `apply-config.bat`, `Register-Tasks.ps1`, `update.bat`, `uninstall.bat` du dossier de données | `apply-config.bat` et `Register-Tasks.ps1` sont supprimés (les deux autres sont inoffensifs : supprimez-les) |
| `config.toml` | clés obsolètes retirées (`install_dir`, `reports_dir`, `*_report_*`), vos autres valeurs conservées (dont `db_path`, et `retention_days` qui devient `raw_retention_days`), ancienne version sauvegardée en `config.toml.v1.bak` |
| `temperatures.db` | **migrée sur place** au premier démarrage, où qu'elle soit : une copie de sécurité `temperatures.db.v1.bak` est écrite à côté, puis l'historique est agrégé par jour |
| Rapports `.md` du dossier `rapports\` | non touchés (supprimez-les si vous n'en voulez plus) |

Si le dossier `\MonitoringAlert\` vide reste visible dans le Planificateur de tâches, il est inoffensif.
L'ancien `update.bat` de la 1.x ne fonctionne plus (il cherchait une archive zip, qui n'est plus publiée) :
utilisez l'installateur.

> **Vos données** : tout l'historique est conservé.
> - Les **agrégats quotidiens** (médiane, min, p95, max par jour, capteur et niveau de charge) sont conservés
>   **sans limite** : ce sont eux qui permettent de suivre la dérive sur toute la durée.
> - Les **mesures brutes** (une toutes les 5 minutes) sont conservées `raw_retention_days` jours. Une installation
>   qui vient de la 1.x garde **sa** durée (`retention_days`, 365 jours par défaut en 1.x) ; une installation
>   neuve garde 90 jours. Mettez `3650` pour tout garder : la base reste petite (de l'ordre de quelques dizaines de
>   Mo par an). Avant toute suppression, les mesures brutes sont résumées dans les agrégats.

### Repartir de zéro (en gardant la base)

Désinstaller depuis Windows en répondant **non** à la suppression des données, puis réinstaller : l'assistant
retrouve la base. Pour nettoyer une 1.x qui ne se lance plus, installez simplement la nouvelle version : elle
supprime l'ancien système.

---

## Le tableau de bord

**<http://127.0.0.1:8787/>** — il répond à une question : *les températures de mon PC ont-elles changé depuis le début ?*
Il est écrit en langage courant ; les termes techniques sont dans « Détails techniques ».

Quatre pages, dans le menu en haut :
- **Accueil** : « Depuis le 8 avril (environ 7 mois de suivi), quand le PC est au repos : 3 composants plus chauds,
  3 stables, 1 plus froid », le changement le plus marqué, puis **une carte par composant** : Processeur, Carte graphique,
  Carte mère, Boîtier, Disque 1, Disque 2… (chacun avec son modèle). Une carte donne la température *au début* (les 30
  premiers jours de suivi), *ces 30 derniers jours*, l'évolution en un mot (*Stable*, *Plus chaud*, *Plus froid*) et une
  mini-courbe colorée selon la tendance. « Stable » veut dire que l'écart ne dépasse pas ce qu'on sait mesurer :
  au moins 1,5 °C (la plupart des capteurs ne mesurent qu'au degré près), davantage pour un capteur qui varie
  beaucoup d'un jour à l'autre. La marge retenue est dans les « Détails techniques » du capteur. Toucher une carte
  ouvre sa courbe.
  Chaque composant est aussi comparé **au reste de la machine** : la carte dit *Comme toute la machine* (il a suivi le
  mouvement général, donc la pièce) ou *Plus que le reste* avec l'écart en degrés (il chauffe seul, donc lui). La phrase
  du haut nomme le composant qui se détache, ou conclut que tout a bougé ensemble.
- **Courbes** : tous les composants sur un même graphique, chacun avec sa couleur ; une case par composant pour le
  masquer et mieux voir les autres. Au choix : les températures, ou l'écart depuis le début. Mois alternés en fond pour
  repérer la saisonnalité, vue « chiffres, mois par mois ».
- **Détails** : tous les capteurs, composant par composant. Cliquer une ligne l'ouvre : une phrase qui explique ses
  chiffres, sa courbe et les « Détails techniques ».
- **État** : la page surveille elle-même que tout fonctionne (relevé des températures, capteurs qui répondent, base de
  données, résumés quotidiens, démarrage automatique, mises à jour) et dit en une phrase ce qui ne va pas ; ce qui est
  conservé, pour combien de temps et où ; la version. Une pastille en haut de chaque page (« ✓ Tout fonctionne », « ⚠ 2
  points à vérifier », « ✕ Un problème empêche le suivi ») et le titre de l'onglet reprennent l'état : un onglet épinglé
  suffit pour voir que tout va bien. Si le programme lui-même ne répond plus, la page le dit.

**Le PC est…** (au repos, charge légère, moyenne, élevée) : on ne compare que des situations identiques. Ce choix est
dans chaque page, juste au-dessus des chiffres qu'il change. « Au repos » est le plus parlant.

En haut à droite, un bouton **Mise à jour disponible** quand une nouvelle version existe (voir plus bas).

Tant qu'il y a moins de **60 jours** de suivi (et 40 jours avec mesures), aucun changement n'est annoncé : la page dit
combien il en faut. Une installation qui vient de la 1.x reprend tout son historique et n'attend pas.

> **Limite connue** : il n'y a pas de capteur de température ambiante. Une saison chaude fait monter légèrement presque
> tous les composants. C'est pourquoi le tableau de bord ne regarde pas seulement l'écart d'un composant, mais son
> **excès** sur la dérive commune à toute la machine (voir « Comment la dérive est calculée ») : tout monter ensemble,
> c'est la pièce ; un seul composant qui se détache, c'est lui. Cette comparaison remplace le capteur ambiant qui
> manque, elle ne l'égale pas : si la machine entière chauffe parce qu'elle est pleine de poussière, la dérive commune
> monte aussi et aucun composant ne se détachera.

Le serveur n'écoute que sur **cette machine** et ne fait que lire. `web_host = "0.0.0.0"` l'ouvre
au réseau local, **sans authentification** : à ne faire que sur un réseau de confiance.

---

## Configuration

`%LOCALAPPDATA%\Programs\Monitoring\config.toml` — toutes les clés sont facultatives. Une installation
qui date de la 1.x garde son dossier `MonitoringAlert` tant que sa configuration s'y trouve.
Après modification : `monitoring.exe restart`.

| Clé | Défaut | Rôle |
|---|---|---|
| `db_path` | `temperatures.db` à côté du fichier | Base SQLite |
| `log_dir` | dossier de la base | Logs (`monitoring.AAAA-MM-JJ.log`, rotation quotidienne) |
| `collect_interval_secs` | `300` | Intervalle de collecte (minimum 60) |
| `raw_retention_days` | `90` | Jours de mesures brutes conservées (minimum 7) |
| `lhm_host` / `lhm_port` | `127.0.0.1` / `8085` | Serveur HTTP de LibreHardwareMonitor |
| `web_host` / `web_port` | `127.0.0.1` / `8787` | Écoute du tableau de bord |
| `check_updates` | `true` | Chercher une nouvelle version une fois par jour sur GitHub (le tableau de bord l'indique) |
| `log_level` | `info` | `error` · `warn` · `info` · `debug` · `trace` |

---

## Ligne de commande

`monitoring.exe` (console). `monitoring-daemon.exe` est la copie sans fenêtre lancée
automatiquement (créée par `install`) ; le Gestionnaire des tâches affiche ce nom.

| Commande | Rôle |
|---|---|
| `install [--in-place] [--db-path <fichier>]` | Ce que fait l'installateur : nettoie une 1.x, migre `config.toml`, démarrage automatique, démarrage. `--in-place` : l'exécutable est déjà à sa place. `--db-path` : base à utiliser |
| `uninstall` | Arrête le processus, retire le démarrage automatique (ne supprime aucun fichier) |
| `start` · `stop` · `restart` | Gère le processus d'arrière-plan |
| `serve` | Collecte + tableau de bord au premier plan |
| `collect` | Une seule collecte (debug) |
| `check` | Vérifie config, base, LHM, tableau de bord, démarrage automatique, reliquats 1.x. Ne modifie rien |
| `db locate [--porcelain]` | Où est la base actuelle et ce qu'elle contient (ne modifie rien) |
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
   des 30 derniers jours calendaires (≥ 10 jours dans la fenêtre). Si le capteur n'a plus rien mesuré à ce
   niveau de charge depuis **plus de 15 jours**, sa dernière médiane glissante ne décrit plus « ces 30
   derniers jours » : elle est gardée mais n'est plus annoncée comme récente, ne porte plus de verdict et ne
   compte plus dans la dérive de la machine. C'est le cas normal d'un niveau de charge qu'on n'atteint plus
   (arrêter de jouer fait taire « charge élevée »).
5. **Écart** = médiane glissante actuelle − référence. **Pente** = droite des moindres carrés sur toutes les
   médianes quotidiennes, en °C/mois.
   **Marge d'incertitude** de cet écart : erreur-type des deux médianes comparées, estimée de façon robuste
   (1,4826 × écart absolu médian), combinée et portée à 95 % (× 1,96). Un écart n'est annoncé que s'il dépasse
   **le plus grand** des deux seuils : 1,5 °C (la résolution des capteurs) et cette marge. Un capteur dont la
   température varie de 3 °C d'un jour à l'autre doit donc bouger plus qu'un disque très stable — sinon la page
   annoncerait chaque mois un changement qui n'est que du bruit. Les médianes quotidiennes sont traitées comme
   indépendantes, ce qu'elles ne sont pas tout à fait (une vague de chaleur dure plusieurs jours) : la marge est
   donc un plancher de l'incertitude réelle. Elle ne sert qu'à **refuser** un verdict, jamais à en accorder un.
6. **Pointes de la journée** : la même comparaison (référence, période récente, marge) refaite sur le **p95
   quotidien** au lieu de la médiane. Un refroidissement qui ne suit plus se voit là en premier : la
   température habituelle bouge à peine pendant que les pointes montent. Quand les pointes montent nettement
   plus que l'habituel, la page du capteur le dit.
7. **Dérive de la machine** = médiane des écarts des composants, une voix par composant (le capteur qui
   représente le composant, pour qu'un processeur à douze cœurs ne pèse pas plus qu'un disque). L'**excès**
   d'un composant = son écart − cette dérive commune.

C'est l'excès qui répond à la question utile, faute de capteur de température ambiante : quand tout monte
ensemble, chaque excès est proche de zéro et c'est la pièce qui a chauffé ; quand un seul composant a un excès
marqué, il chauffe pour une raison qui lui est propre — poussière, pâte thermique ou ventilateur. Il faut au
moins **3 composants** avec assez de recul pour que cette comparaison ait un sens ; en dessous, le tableau de
bord le dit au lieu de conclure.

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

- **`monitoring.exe check`** : état complet, sans rien modifier.
- **En-tête du tableau de bord** : dernière mesure et erreur éventuelle.
- **Logs** : `monitoring.AAAA-MM-JJ.log` dans `log_dir`. Le collecteur est relancé automatiquement
  s'il échoue, et un échec de collecte n'arrête pas le processus.
- Si le processus ne tourne pas (port déjà pris, base inaccessible), l'erreur est dans le log : le
  processus d'arrière-plan n'a pas de console.

---

## Développement

```
backend/    Rust : la bibliothèque, la CLI, le démon et le serveur web
frontend/   TypeScript (Vite + uPlot) : le tableau de bord
installer/  monitoring.iss (installateur Inno Setup)
tools/      release.sh
```

### Architecture du backend (hexagonale simplifiée)

```
domain/       code pur, sans E/S : modèle, calcul de dérive, composants du PC, vocabulaire, pages du tableau de bord
application/  cas d'usage (usecases/ : un fichier par action, `get_overview`, `get_sensor`…) + ports (traits) : collecte, boucle, installation
adapters/     le technique : sqlite, lhm (HTTP), http (serveur web, un fichier par endpoint), config, logging, windows, pe, deploy
daemon.rs     racine de composition : branche les adapters sur les cas d'usage
```

Les dépendances ne vont que vers l'intérieur (adapters → application → domain) ; `tests/architecture.rs` le vérifie.

Une requête suit toujours le même chemin : **un endpoint** (`adapters/http/endpoints/sensor.rs`) lit la requête et appelle
**un cas d'usage** (`application/usecases/get_sensor.rs`), qui lit ce qu'il lui faut par les ports et appelle **le code
métier** (`domain/dashboard/sensor.rs`). Le backend décide (regroupement, verdicts, phrases, nombres écrits en français),
le frontend affiche.

### Commandes

```bash
# Backend (depuis backend/)
cargo test
cargo clippy --all-targets --all-features -- -D warnings
cargo clippy --target x86_64-pc-windows-gnu --all-targets -- -D warnings   # code Windows, depuis Linux

# Frontend (depuis frontend/)
npm ci
npm run dev        # proxy /api vers 127.0.0.1:8787 (lancer `monitoring serve` à côté)
npm run build      # produit frontend/dist, embarqué dans l'exécutable

# Exécutable complet
npm run build --prefix frontend && cargo build --release --manifest-path backend/Cargo.toml
```

Le frontend est embarqué dans l'exécutable par `backend/build.rs`. Sans build du frontend, `cargo test`
fonctionne (page de remplacement) ; un `cargo build --release` est refusé
(`MONITORING_ALERT_ALLOW_NO_FRONTEND=1` pour passer outre).

### Voir le tableau de bord en local

```bash
tools/demo.sh                              # données de démonstration (14 mois, plusieurs disques, une dérive lente)
tools/demo.sh /mnt/e/monitoring.db/temperatures.db   # vos propres données : la base est COPIÉE, jamais modifiée
tools/demo.sh --update [base]              # ouvre aussi une 2e adresse qui simule « une mise à jour est disponible »
```

Le script construit le tableau de bord et le programme, lance un faux LibreHardwareMonitor qui rejoue les capteurs de la
base, puis le vrai programme, et affiche l'adresse (`http://127.0.0.1:18797/`, ou `DEMO_PORT`). Ctrl+C arrête tout et
supprime la copie. Pour voir l'état « problème », arrêtez le faux LibreHardwareMonitor (le script donne la commande).
`npm run dev` (dans `frontend/`) recharge la page à chaque modification, avec le programme lancé à côté.

Conventions du projet (commits, branche, tests, documentation) : voir [CLAUDE.md](CLAUDE.md).
