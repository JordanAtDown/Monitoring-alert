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
- Types de l'API dans `src/api.ts` : ils reflètent le JSON de `backend/src/application/queries.rs`. Les
  changer ensemble.

## Graphiques (skill `dataviz` avant tout code de graphique)

- Couleurs = **jetons CSS** (`--series-1`, `--diverge-warm`…) définis dans `style.css` pour le clair et le
  sombre ; le canvas les lit via `theme.ts` au moment du dessin. Pas de couleur codée en dur dans le JS.
- Palette validée avec `validate_palette.js` du skill `dataviz` (une série catégorielle + pôles divergents).
  Revalider si on ajoute une série.
- Un seul axe y. Pas de double axe.
- Le texte porte les jetons d'encre, jamais la couleur de la série.
- Un graphique a toujours : légende (≥ 2 séries), infobulle, vue tableau, et fonctionne au clavier là où
  c'est possible.
- Les mois en alternance claire/foncée sont voulus : sans capteur ambiant, la saisonnalité doit se voir.
- Pas d'alerte, pas de vocabulaire d'urgence (« critique », « danger »…) : l'écart est un chiffre à regarder.

## Limite connue

L'interface n'a été vérifiée que par typecheck, build et tests de l'API : pas de rendu visuel automatisé.
Ouvrir le tableau de bord réel après un changement de rendu.
