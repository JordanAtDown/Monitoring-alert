// Curves: every component on one chart, each one can be switched off to see the others better.

import type { CurveSeries, Curves, CurvesMode } from "../api";
import { drawChart, type ChartSeries } from "../chart";
import { clear, h } from "../dom";
import { colorValue, colorVar, disposeCharts, state, trackChart, type Actions } from "../state";
import { loadFilter, textTable } from "../widgets";

const MODES: { id: CurvesMode; text: string }[] = [
  { id: "temp", text: "Températures" },
  { id: "delta", text: "Écart depuis le début" },
];

function chartSeries(s: CurveSeries): ChartSeries {
  return { id: s.key, label: s.title, color: colorValue(s.slot), values: new Map(s.points.map((p) => [p.day, p.value])) };
}

export function renderCurves(host: HTMLElement, data: Curves, actions: Actions): void {
  clear(host);
  const card = h("section", { id: "curves", class: "card" }, h("h2", {}, "Courbes"));
  host.append(card);
  if (state.overview) card.append(loadFilter(state.overview.load, actions));
  const all = data.series;
  if (all.every((s) => s.points.length === 0 && s.note === "pas de mesure")) {
    card.append(h("p", { class: "muted" }, "Pas encore de journée complète avec assez de mesures à ce niveau de charge."));
    return;
  }
  card.append(h("p", { class: "muted" }, "Chaque ligne montre la température habituelle d'un composant, mois après mois. Décochez un composant pour mieux voir les autres."));

  const picker = h("ul", { class: "picker" });
  const chart = h("div", { class: "chart", role: "img", "aria-label": "Courbes de la température de chaque composant dans le temps" });
  const tableSlot = h("div");
  const empty = h("p", { class: "muted", hidden: true }, "Aucun composant affiché : cochez-en au moins un.");

  const visible = (): { s: CurveSeries; index: number }[] =>
    all.map((s, index) => ({ s, index })).filter(({ s }) => s.available && !state.hidden.has(s.key));

  const redraw = (): void => {
    clear(chart);
    for (const li of picker.querySelectorAll<HTMLElement>("li")) {
      const s = all[Number(li.dataset.index)];
      const input = li.querySelector("input") as HTMLInputElement;
      input.disabled = !s.available;
      input.checked = s.available && !state.hidden.has(s.key);
      li.classList.toggle("off", !input.checked);
      (li.querySelector(".series-now") as HTMLElement).textContent = s.note ?? s.now ?? "—";
    }
    const shown = visible();
    empty.hidden = shown.length > 0;
    chart.hidden = shown.length === 0;
    clear(tableSlot);
    if (shown.length === 0) return;
    // A single line shows where it began; in "change" mode every line shares the beginning (0).
    const reference = data.mode === "delta" ? shown[0].s.reference : shown.length === 1 ? shown[0].s.reference : null;
    trackChart(drawChart(chart, { series: shown.map(({ s }) => chartSeries(s)), unit: data.mode, reference }));
    tableSlot.append(
      h(
        "details",
        {},
        h("summary", {}, "Voir les chiffres, mois par mois"),
        textTable(["Mois", ...shown.map(({ s }) => s.title)], data.months.map((m) => [m.label, ...shown.map(({ index }) => m.cells[index] ?? "—")])),
      ),
    );
  };
  const refresh = (): void => {
    disposeCharts();
    redraw();
  };

  all.forEach((s, index) => {
    const input = h("input", { type: "checkbox" });
    input.addEventListener("change", () => {
      if (input.checked) state.hidden.delete(s.key);
      else state.hidden.add(s.key);
      refresh();
    });
    const swatch = h("span", { class: "swatch", "aria-hidden": "true" });
    swatch.style.background = colorVar(s.slot);
    picker.append(
      h(
        "li",
        { "data-index": String(index) },
        h("label", { class: "series-toggle" }, input, swatch, h("span", { class: "series-text" }, h("span", { class: "series-name" }, s.title), h("span", { class: "series-sensor" }, s.sensor_name)), h("span", { class: "series-now" })),
      ),
    );
  });

  const showAll = h("button", { type: "button", class: "link-button" }, "Tout afficher");
  const none = h("button", { type: "button", class: "link-button" }, "Tout masquer");
  showAll.addEventListener("click", () => {
    state.hidden.clear();
    refresh();
  });
  none.addEventListener("click", () => {
    for (const s of all) state.hidden.add(s.key);
    refresh();
  });

  const modes = h("div", { class: "segmented", role: "group", "aria-label": "Ce que montrent les courbes" });
  for (const m of MODES) {
    const button = h("button", { type: "button", class: "segment", "aria-pressed": String(state.mode === m.id) }, m.text);
    button.addEventListener("click", () => {
      if (state.mode !== m.id) actions.setMode(m.id);
    });
    modes.append(button);
  }

  card.append(
    h("div", { class: "curve-controls" }, modes, h("span", { class: "bulk" }, showAll, " · ", none)),
    picker,
    h("p", { class: "hint" }, data.hint),
    empty,
    chart,
    h("p", { class: "hint" }, "Glissez sur la courbe pour zoomer, double-cliquez pour revenir. Les bandes grises alternées marquent les mois."),
    tableSlot,
  );
  redraw();
}
