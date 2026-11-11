// Home: the answer in a few words, then one card per component of the PC.

import { clear, h } from "../dom";
import { fmtDay, fmtInt, fmtSigned, fmtTemp } from "../format";
import { hashFor, state, type Actions } from "../state";
import { LOADS, MIN_SPAN_DAYS, spanInWords, STABLE_BAND, type Verdict } from "../text";
import { componentVerdict, loadFilter, plural, sparkline, verdictChip } from "../widgets";

const TILE_WORDS: Record<Verdict, [string, string]> = {
  warmer: ["composant plus chaud qu'au début", "composants plus chauds qu'au début"],
  stable: ["composant stable", "composants stables"],
  cooler: ["composant plus froid qu'au début", "composants plus froids qu'au début"],
  unknown: ["composant sans assez de recul", "composants sans assez de recul"],
};

function tile(verdict: Verdict, n: number): HTMLElement {
  return h("div", { class: `tile tile-${verdict}` }, h("span", { class: "tile-number" }, fmtInt(n)), h("span", { class: "tile-words" }, plural(n, TILE_WORDS[verdict][0], TILE_WORDS[verdict][1])));
}

function summaryCard(actions: Actions): HTMLElement {
  const card = h("section", { id: "verdict", class: "card", "aria-live": "polite" }, h("h2", {}, "En résumé"), loadFilter(actions));
  const load = LOADS.find((l) => l.id === state.load);
  const mains = state.components.map((c) => c.main);
  const known = mains.filter((m) => m.sufficient && m.delta !== null);

  if (mains.length === 0) {
    card.append(h("p", { class: "lead" }, "Aucune mesure résumée pour l'instant."), h("p", { class: "muted" }, "Les premiers résumés apparaissent le lendemain de la première collecte."));
    return card;
  }
  if (known.length === 0) {
    const days = Math.max(...mains.map((m) => m.days));
    card.append(
      h("p", { class: "lead" }, "Pas encore assez d'historique pour comparer."),
      h("p", { class: "muted" }, `Pour dire si une température a changé, il faut au moins ${MIN_SPAN_DAYS} jours de suivi dont 40 jours avec des mesures ${load?.when}. Pour l'instant : ${fmtInt(days)} jour${days > 1 ? "s" : ""} de mesures.`),
    );
    return card;
  }

  const span = Math.max(...known.map((m) => m.span_days));
  const since = state.status?.db.history_since;
  card.append(h("p", { class: "lead" }, `${since ? `Depuis le ${fmtDay(since)}` : "Depuis le début"} (${spanInWords(span)} de suivi), ${load?.when} :`));

  const counts: Record<Verdict, number> = { warmer: 0, stable: 0, cooler: 0, unknown: 0 };
  for (const c of state.components) counts[componentVerdict(c)] += 1;
  const tiles = h("div", { class: "tiles" }, tile("warmer", counts.warmer), tile("stable", counts.stable), tile("cooler", counts.cooler));
  if (counts.unknown > 0) tiles.append(tile("unknown", counts.unknown));
  card.append(tiles);

  const top = [...state.components].filter((c) => c.main.sufficient && c.main.delta !== null).sort((a, b) => Math.abs(b.main.delta as number) - Math.abs(a.main.delta as number))[0];
  const topDelta = top.main.delta as number;
  card.append(
    h(
      "p",
      {},
      [
        Math.abs(topDelta) < STABLE_BAND ? "Aucun composant n'a vraiment changé de température." : `Le changement le plus marqué : ${top.title} (${fmtSigned(topDelta)}).`,
        Math.abs(topDelta) < 3 ? "Les écarts restent faibles (moins de 3 °C)." : "Une saison chaude peut expliquer une partie de l'écart.",
      ].join(" "),
    ),
  );
  return card;
}

function componentsGrid(): HTMLElement {
  const section = h("section", { id: "components", class: "card" }, h("h2", {}, "Vos composants"));
  if (state.components.length === 0) {
    section.append(h("p", { class: "muted" }, "Rien à afficher pour ce niveau de charge pour l'instant."));
    return section;
  }
  section.append(h("p", { class: "muted" }, "Touchez un composant pour voir sa courbe."));
  const grid = h("div", { class: "comp-grid" });
  for (const c of state.components) {
    const m = c.main;
    grid.append(
      h(
        "a",
        { class: "comp-card", href: hashFor("curves", { only: c.key }) },
        h("span", { class: "comp-head" }, h("span", { class: "comp-title" }, c.title), m.sufficient ? verdictChip(m) : h("span", { class: "chip chip-unknown" }, "Pas assez de recul")),
        h("span", { class: "comp-sub" }, c.subtitle ?? " "),
        h(
          "span",
          { class: "comp-figures" },
          h("span", { class: "figure" }, h("span", { class: "figure-label" }, "Au début"), h("span", { class: "figure-value" }, m.sufficient && m.baseline !== null ? fmtTemp(m.baseline) : "—")),
          h("span", { class: "figure" }, h("span", { class: "figure-label" }, "Ces 30 derniers jours"), h("span", { class: "figure-value" }, m.current === null ? "—" : fmtTemp(m.current))),
          h("span", { class: "spark" }, sparkline(m.sparkline, componentVerdict(c))),
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
      h("li", {}, `On parle de « stable » quand la différence est inférieure à ${String(STABLE_BAND).replace(".", ",")} °C : la plupart des capteurs ne mesurent qu'au degré près.`),
      h("li", {}, "On compare toujours des situations identiques (« Le PC est… ») : la température au repos avec celle au repos, pas avec celle d'un jeu."),
      h("li", {}, "Une saison chaude fait monter légèrement presque tous les composants. Un composant qui chauffe seul, lui, mérite qu'on y regarde de plus près."),
    ),
  );
}

export function renderHome(host: HTMLElement, actions: Actions): void {
  clear(host);
  host.append(summaryCard(actions), componentsGrid(), helpBlock());
}
