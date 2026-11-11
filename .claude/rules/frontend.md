---
paths:
  - "frontend/**"
---

# Frontend (TypeScript, Vite, uPlot)

- **Sans framework** : TypeScript + uPlot. Pas de dépendance de plus sans la justifier dans le commit.
- **Les noms de matériel et de capteurs viennent de la machine : jamais de `innerHTML`.** Construire le DOM
  avec `h()` / `svg()` (`src/dom.ts`), qui n'insèrent que du texte.
- Les fichiers de sortie ont des **noms fixes** (`assets/app.js`, `assets/app.css`, `index.html`,
  `favicon.svg`) : `backend/build.rs` les embarque par nom. Ne pas ajouter d'autre fichier à embarquer
  sans modifier `build.rs` et `adapters/http/assets.rs`. Le serveur les envoie en `no-store`.
- `npm run build` (typecheck + build) doit passer ; `npm run dev` proxifie `/api` vers `127.0.0.1:8787`.
- Types de l'API dans `src/api.ts` : ils reflètent le JSON de `backend/src/domain/dashboard/` (servi par
  `backend/src/application/usecases/`). Les changer ensemble.
- **Le frontend n'a aucune règle métier** : ni seuil, ni vocabulaire, ni regroupement, ni mise en forme de nombre. Tout
  arrive écrit par le backend (`/api/overview`, `/api/curves`, `/api/sensor`, `/api/status`, `/api/update`) ; la page
  va chercher, met en page, colore et dessine. Besoin d'un mot, d'un seuil, d'une phrase : c'est dans le backend
  (`domain/wording`, `domain/dashboard`). Seul le graphique formate des nombres (axes, infobulle).

## Organisation

Quatre pages (`src/views/` : `home`, `curves`, `details`, `status`), un menu par liens `#page=…` (l'adresse garde la page, le
niveau de charge et le capteur ouvert). `main.ts` = coque, routage et flux de données ; `state.ts` = état partagé ;
`widgets.ts` = morceaux partagés. Le filtre « Le PC est… » est **dans chaque carte** qu'il change, jamais dans une barre.

## Graphiques (skill `dataviz` avant tout code de graphique)

- Couleurs = **jetons CSS** (`--series-1`, `--diverge-warm`…) définis dans `style.css` pour le clair et le
  sombre ; le canvas les lit via `theme.ts` au moment du dessin. Pas de couleur codée en dur dans le JS.
- Palette de 8 séries catégorielles (`--series-1…8`) validée avec `validate_palette.js` du skill `dataviz`, clair et
  sombre. **Un composant garde sa couleur** (rang dans la liste, pas dans la sélection) ; au-delà de 8, gris. Les mini-courbes
  prennent le pôle chaud / froid / gris de la tendance, le mot de la tendance restant à côté.
- Un seul axe y. Pas de double axe.
- Le texte porte les jetons d'encre, jamais la couleur de la série.
- Un graphique a toujours : légende (ici les cases à cocher), infobulle, vue tableau, et fonctionne au clavier là où
  c'est possible.
- Les mois en alternance claire/foncée sont voulus : sans capteur ambiant, la saisonnalité doit se voir.
- Pas d'alerte, pas de vocabulaire d'urgence (« critique », « danger »…) : l'écart est un chiffre à regarder.

## Langage courant

L'interface s'adresse à quelqu'un qui n'a **aucune notion de statistiques**. Les mots de la vue principale sont écrits
par le backend (`domain/wording`, `domain/dashboard`) : « au début », « ces 30 derniers jours », « Plus chaud / Stable /
Plus froid ». Médiane, p95, pente, moindres carrés : seulement dans « Détails techniques ». Une phrase vaut mieux qu'un
chiffre seul. Les accords (singulier/pluriel) et la ponctuation se vérifient dans les tests du **backend** ; ceux du
frontend vérifient que la page affiche ce qu'on lui donne, mot pour mot.

## Tests

`npm test` exécute la vraie page dans un navigateur simulé (jsdom) avec des réponses d'API enregistrées
(`test/fixtures/` : `overview-idle`, `curves-idle-*`, `sensor-disk1`, `status`, enregistrées sur un vrai backend) :
texte affiché, cases à cocher, bouton de mise à jour, liens hostiles.
uPlot est remplacé par une doublure (pas de canvas) : on teste **ce que dit la page**, pas ses pixels. Régénérer les
fixtures depuis un vrai backend si le JSON de l'API change. Un changement de wording = un test qui le dit.

## Limite connue

Le rendu graphique (couleurs, disposition, courbe) n'est vérifié ni par `npm test` ni par la CI : ouvrir le tableau de
bord réel après un changement de style ou de graphique.
