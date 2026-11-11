// Details: every sensor of every component, with the figures; one can be opened to read its story.

import type { ComponentView, SensorPage, SensorView } from "../api";
import { drawChart } from "../chart";
import { clear, h } from "../dom";
import { colorValue, sensorKey, state, trackChart, type Actions } from "../state";
import { loadFilter, sparkline, textTable, verdictChip } from "../widgets";

const isSelected = (row: SensorView): boolean => state.selected?.hardware === row.hardware && state.selected?.sensor === row.sensor;

function legendItem(label: string): HTMLElement {
  const className = label.startsWith("tirets") ? "key-dash" : "key-line";
  return h("li", {}, h("span", { class: `key ${className}`, "aria-hidden": "true" }), label);
}

/** The opened sensor: its sentence, its curve, its figures. The curve is drawn once it is in the page. */
function detail(row: SensorView): { element: HTMLElement; draw: () => void } {
  const element = h("div", { class: "detail" });
  const page: SensorPage | null | undefined = state.sensors.get(sensorKey(row));
  element.append(h("h3", {}, row.name));
  if (page === undefined) {
    element.append(h("p", { class: "muted" }, "Chargement…"));
    return { element, draw: () => {} };
  }
  if (page === null) {
    element.append(h("p", { class: "story" }, "Impossible de charger la courbe de ce capteur."));
    return { element, draw: () => {} };
  }
  element.append(h("p", { class: "muted" }, page.subtitle));
  for (const line of page.story) {
    element.append(h("p", { class: line.kind }, ...line.spans.map((s) => (s.strong ? h("strong", {}, s.text) : s.text))));
  }
  if (page.rolling.length === 0) return { element, draw: () => {} };

  const chart = h("div", { class: "chart", role: "img", "aria-label": `Courbe de la température de ${page.name} dans le temps` });
  element.append(chart, h("ul", { class: "legend" }, ...page.legend.map(legendItem)), h("p", { class: "hint" }, "Glissez sur la courbe pour zoomer, double-cliquez pour revenir. Les bandes grises alternées marquent les mois."));
  if (page.seasons.length > 0) {
    // The same month a year apart: the comparison the season cannot distort, so it is open.
    element.append(
      h(
        "details",
        { open: true },
        h("summary", {}, "Le même mois, un an avant"),
        textTable(
          ["Mois", "Un an avant", "Ce mois-ci", "Écart"],
          page.seasons.map((s) => [s.label, s.before, s.after, s.change]),
        ),
      ),
    );
  }
  element.append(
    h(
      "details",
      {},
      h("summary", {}, "Voir les chiffres, mois par mois"),
      textTable(["Mois", "Jours mesurés", "Température habituelle", "La plus basse", "La plus haute"], page.months.map((m) => [m.label, String(m.days), m.usual, m.lowest, m.highest])),
    ),
  );
  if (page.technical.length > 0) {
    const list = h("dl", { class: "tech-list" });
    for (const f of page.technical) list.append(h("dt", {}, f.label), h("dd", {}, f.text));
    element.append(h("details", {}, h("summary", {}, "Détails techniques"), list));
  }
  const draw = (): void =>
    trackChart(
      drawChart(chart, {
        series: [{ id: row.sensor, label: page.name, color: colorValue(page.slot ?? 0), values: new Map(page.rolling.map((p) => [p.day, p.value])) }],
        unit: "temp",
        reference: page.reference,
      }),
    );
  return { element, draw };
}

function sensorRow(row: SensorView, actions: Actions): HTMLElement {
  const selected = isSelected(row);
  const tr = h("tr", { tabindex: "0", "aria-expanded": String(selected), class: selected ? "selected" : undefined });
  tr.append(
    h("th", { scope: "row" }, h("span", { class: "sensor" }, row.name), row.tech ? h("span", { class: "tech" }, row.tech) : null),
    h("td", { class: "num" }, row.baseline ?? "—"),
    h("td", { class: "num" }, row.current ?? "—"),
    h("td", {}, verdictChip(row.verdict)),
    h("td", { class: "spark" }, sparkline(row.sparkline, row.verdict.id)),
  );
  const toggle = (): void => actions.select(selected ? null : { hardware: row.hardware, sensor: row.sensor });
  tr.addEventListener("click", toggle);
  tr.addEventListener("keydown", (e) => {
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      toggle();
    }
  });
  return tr;
}

function table(component: ComponentView, actions: Actions): { element: HTMLElement; draw: () => void } {
  let draw: () => void = () => {};
  const body = h("tbody");
  for (const row of component.sensors) {
    body.append(sensorRow(row, actions));
    if (isSelected(row)) {
      const d = detail(row);
      draw = d.draw;
      body.append(h("tr", { class: "detail-row" }, h("td", { colspan: "5" }, d.element)));
    }
  }
  const element = h(
    "div",
    { class: "table-wrap" },
    h(
      "table",
      {},
      h("caption", {}, component.title, component.subtitle ? h("span", { class: "caption-sub" }, ` — ${component.subtitle}`) : null),
      h("thead", {}, h("tr", {}, h("th", { scope: "col" }, "Capteur"), h("th", { scope: "col", class: "num" }, "Au début"), h("th", { scope: "col", class: "num" }, "Ces 30 derniers jours"), h("th", { scope: "col" }, "Évolution"), h("th", { scope: "col", class: "spark" }, "Courbe"))),
      body,
    ),
  );
  return { element, draw };
}

export function renderDetails(host: HTMLElement, actions: Actions): void {
  clear(host);
  const card = h("section", { id: "components", class: "card" }, h("h2", {}, "Détail de chaque capteur"));
  host.append(card);
  const o = state.overview;
  if (!o) return;
  card.append(loadFilter(o.load, actions));
  if (o.components.length === 0) {
    card.append(h("p", { class: "muted" }, "Rien à afficher pour ce niveau de charge pour l'instant."));
    return;
  }
  card.append(h("p", { class: "muted" }, "Cliquez sur une ligne pour lire l'histoire du capteur et voir sa courbe."));
  const draws: (() => void)[] = [];
  for (const component of o.components) {
    const t = table(component, actions);
    card.append(t.element);
    draws.push(t.draw);
  }
  for (const draw of draws) draw();
}
