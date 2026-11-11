// Curves: every component on one chart, each one can be switched off to see the others better.

import type { Trend } from "../api";
import { drawChart, type ChartSeries } from "../chart";
import { sensorLabel, type Component } from "../components";
import { clear, h } from "../dom";
import { fmtSigned, fmtTemp } from "../format";
import { colorValue, colorVar, disposeCharts, state, trackChart, trendOf, type Actions, type Mode } from "../state";
import { LOADS } from "../text";
import { loadFilter, monthlyMatrix } from "../widgets";

interface Line {
  component: Component;
  index: number;
  trend: Trend | null;
}

function lines(): Line[] {
  return state.components.map((component, index) => {
    const trend = trendOf(component.main.hardware, component.main.sensor) ?? null;
    return { component, index, trend: trend && trend.points.length > 0 ? trend : null };
  });
}

/** Can this line be drawn in the current mode? "Change since the beginning" needs a beginning. */
function drawable(line: Line): boolean {
  return line.trend !== null && (state.mode === "temp" || line.trend.baseline !== null);
}

function seriesOf(line: Line): ChartSeries {
  const trend = line.trend as Trend;
  const base = state.mode === "delta" ? (trend.baseline as number) : 0;
  return {
    id: line.component.key,
    label: line.component.title,
    color: colorValue(line.index),
    values: new Map(trend.rolling.map((r) => [r.day, r.value - base])),
  };
}

function segmented(label: string, options: { id: Mode; text: string }[], onPick: (m: Mode) => void): HTMLElement {
  const group = h("div", { class: "segmented", role: "group", "aria-label": label });
  for (const o of options) {
    const button = h("button", { type: "button", class: "segment", "aria-pressed": String(state.mode === o.id) }, o.text);
    button.addEventListener("click", () => onPick(o.id));
    group.append(button);
  }
  return group;
}

export function renderCurves(host: HTMLElement, actions: Actions): void {
  clear(host);
  const load = LOADS.find((l) => l.id === state.load);
  const all = lines();

  const card = h("section", { id: "curves", class: "card" }, h("h2", {}, "Courbes"), loadFilter(actions));
  host.append(card);
  if (all.every((l) => l.trend === null)) {
    card.append(h("p", { class: "muted" }, "Pas encore de journée complète avec assez de mesures à ce niveau de charge."));
    return;
  }
  card.append(
    h(
      "p",
      { class: "muted" },
      `Chaque ligne montre la température habituelle d'un composant ${load?.when}, mois après mois. Décochez un composant pour mieux voir les autres.`,
    ),
  );

  const picker = h("ul", { class: "picker" });
  const chart = h("div", { class: "chart", role: "img", "aria-label": "Courbes de la température de chaque composant dans le temps" });
  const tableSlot = h("div");
  const empty = h("p", { class: "muted", hidden: true }, "Aucun composant affiché : cochez-en au moins un.");

  const visible = (): Line[] => all.filter((l) => drawable(l) && !state.hidden.has(l.component.key));

  const redraw = (): void => {
    clear(chart);
    for (const li of picker.querySelectorAll<HTMLElement>("li")) {
      const line = all[Number(li.dataset.index)];
      const input = li.querySelector("input") as HTMLInputElement;
      const ok = drawable(line);
      input.disabled = !ok;
      input.checked = ok && !state.hidden.has(line.component.key);
      li.classList.toggle("off", !input.checked);
      const now = li.querySelector(".series-now") as HTMLElement;
      now.textContent = !line.trend ? "pas de mesure" : !ok ? "pas assez de recul" : state.mode === "delta" ? (line.component.main.delta === null ? "—" : fmtSigned(line.component.main.delta)) : line.component.main.current === null ? "—" : fmtTemp(line.component.main.current);
    }
    const shown = visible();
    empty.hidden = shown.length > 0;
    chart.hidden = shown.length === 0;
    clear(tableSlot);
    if (shown.length === 0) return;
    const reference =
      state.mode === "delta"
        ? { value: 0, label: "Au début" }
        : shown.length === 1 && shown[0].trend?.baseline != null
          ? { value: shown[0].trend.baseline, label: `Au début : ${fmtTemp(shown[0].trend.baseline)}` }
          : null;
    trackChart(drawChart(chart, { series: shown.map(seriesOf), unit: state.mode, reference }));
    tableSlot.append(
      h(
        "details",
        {},
        h("summary", {}, "Voir les chiffres, mois par mois"),
        monthlyMatrix(shown.map((l) => ({ label: l.component.title, trend: l.trend as Trend, baseline: l.trend?.baseline ?? null })), state.mode === "delta"),
      ),
    );
  };

  const refresh = (): void => {
    disposeCharts();
    redraw();
  };

  for (const line of all) {
    const input = h("input", { type: "checkbox" });
    input.addEventListener("change", () => {
      if (input.checked) state.hidden.delete(line.component.key);
      else state.hidden.add(line.component.key);
      refresh();
    });
    const swatch = h("span", { class: "swatch", "aria-hidden": "true" });
    swatch.style.background = colorVar(line.index);
    picker.append(
      h(
        "li",
        { "data-index": String(line.index) },
        h(
          "label",
          { class: "series-toggle" },
          input,
          swatch,
          h("span", { class: "series-text" }, h("span", { class: "series-name" }, line.component.title), h("span", { class: "series-sensor" }, sensorLabel(line.component.main.sensor, line.component.kind).main)),
          h("span", { class: "series-now" }),
        ),
      ),
    );
  }

  const showAll = h("button", { type: "button", class: "link-button" }, "Tout afficher");
  const none = h("button", { type: "button", class: "link-button" }, "Tout masquer");
  showAll.addEventListener("click", () => {
    state.hidden.clear();
    refresh();
  });
  none.addEventListener("click", () => {
    for (const l of all) state.hidden.add(l.component.key);
    refresh();
  });

  const modes = segmented(
    "Ce que montrent les courbes",
    [
      { id: "temp", text: "Températures" },
      { id: "delta", text: "Écart depuis le début" },
    ],
    (m) => {
      if (state.mode === m) return;
      state.mode = m;
      for (const b of modes.querySelectorAll("button")) b.setAttribute("aria-pressed", String(b.textContent === (m === "temp" ? "Températures" : "Écart depuis le début")));
      hint.textContent = modeHint();
      refresh();
    },
  );
  const modeHint = (): string =>
    state.mode === "temp" ? "Température habituelle de chaque composant." : "Combien de degrés de plus ou de moins qu'au début : 0 = comme au début. Pratique pour comparer des composants de températures différentes.";
  const hint = h("p", { class: "hint" }, modeHint());

  card.append(h("div", { class: "curve-controls" }, modes, h("span", { class: "bulk" }, showAll, " · ", none)), picker, hint, empty, chart, h("p", { class: "hint" }, "Glissez sur la courbe pour zoomer, double-cliquez pour revenir. Les bandes grises alternées marquent les mois."), tableSlot);
  redraw();
}
