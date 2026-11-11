// Home: the answer in a few words, then one card per component of the PC.

import { clear, h } from "../dom";
import { hashFor, state, type Actions } from "../state";
import { loadFilter, sparkline, verdictChip } from "../widgets";

function summaryCard(actions: Actions): HTMLElement {
  const o = state.overview;
  const card = h("section", { id: "verdict", class: "card", "aria-live": "polite" }, h("h2", {}, "En résumé"));
  if (!o) return card;
  card.append(loadFilter(o.load, actions));
  const { lead, detail, tiles } = o.headline;
  card.append(h("p", { class: "lead" }, lead));
  if (tiles.length > 0) {
    card.append(
      h("div", { class: "tiles" }, ...tiles.map((t) => h("div", { class: `tile tile-${t.verdict}` }, h("span", { class: "tile-number" }, String(t.count)), h("span", { class: "tile-words" }, t.words)))),
    );
  }
  if (detail) card.append(h("p", o.headline.state === "ok" ? {} : { class: "muted" }, detail));
  return card;
}

function componentsGrid(): HTMLElement {
  const section = h("section", { id: "components", class: "card" }, h("h2", {}, "Vos composants"));
  const components = state.overview?.components ?? [];
  if (components.length === 0) {
    section.append(h("p", { class: "muted" }, "Rien à afficher pour ce niveau de charge pour l'instant."));
    return section;
  }
  section.append(h("p", { class: "muted" }, "Touchez un composant pour voir sa courbe."));
  const grid = h("div", { class: "comp-grid" });
  for (const c of components) {
    const m = c.main;
    grid.append(
      h(
        "a",
        { class: "comp-card", href: hashFor("curves", { only: c.key }) },
        h("span", { class: "comp-head" }, h("span", { class: "comp-title" }, c.title), verdictChip(m.verdict)),
        h("span", { class: "comp-sub" }, c.subtitle ?? " "),
        h(
          "span",
          { class: "comp-figures" },
          h("span", { class: "figure" }, h("span", { class: "figure-label" }, "Au début"), h("span", { class: "figure-value" }, m.baseline ?? "—")),
          h("span", { class: "figure" }, h("span", { class: "figure-label" }, "Ces 30 derniers jours"), h("span", { class: "figure-value" }, m.current ?? "—")),
          h("span", { class: "spark" }, sparkline(m.sparkline, m.verdict.id)),
        ),
      ),
    );
  }
  section.append(grid);
  return section;
}

function helpBlock(): HTMLElement {
  return h(
    "details",
    { id: "help", class: "help" },
    h("summary", {}, "Comment lire cette page ?"),
    h(
      "ol",
      {},
      h("li", {}, "Chaque composant de votre PC (processeur, carte graphique, disques…) a une carte avec sa température."),
      h("li", {}, h("strong", {}, "Au début"), " est la température habituelle du composant pendant ses 30 premiers jours de suivi. ", h("strong", {}, "Ces 30 derniers jours"), " est sa température habituelle récemment."),
      h("li", {}, "« Stable » veut dire que la différence est faible : la plupart des capteurs ne mesurent qu'au degré près."),
      h("li", {}, "On compare toujours des situations identiques (« Le PC est… ») : la température au repos avec celle au repos, pas avec celle d'un jeu."),
      h("li", {}, "Une saison chaude fait monter légèrement presque tous les composants. Un composant qui chauffe seul, lui, mérite qu'on y regarde de plus près."),
    ),
  );
}

export function renderHome(host: HTMLElement, actions: Actions): void {
  clear(host);
  host.append(summaryCard(actions), componentsGrid(), helpBlock());
}
