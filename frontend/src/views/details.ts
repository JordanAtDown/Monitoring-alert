// Details: every sensor of every component, with the figures; one can be opened to read its story.

import type { Summary, Trend } from "../api";
import { drawChart } from "../chart";
import { sensorLabel, type Component } from "../components";
import { clear, h } from "../dom";
import { fmtDay, fmtInt, fmtSigned, fmtTemp } from "../format";
import { colorValue, state, trackChart, trendOf, type Actions } from "../state";
import { LOADS, MIN_SPAN_DAYS, verdictOf, type Verdict } from "../text";
import { loadFilter, monthlyTable, sparkline, verdictChip } from "../widgets";

const isSelected = (row: Summary): boolean => state.selected?.hardware === row.hardware && state.selected?.sensor === row.sensor;

/** What the numbers mean, as sentences. */
function story(trend: Trend): HTMLElement[] {
  if (trend.points.length === 0) {
    return [h("p", { class: "story" }, "Pas encore de journée complète avec assez de mesures pour ce capteur à ce niveau de charge.")];
  }
  if (!trend.sufficient || trend.baseline === null || trend.delta === null) {
    return [
      h(
        "p",
        { class: "story" },
        `Pour l'instant : ${fmtInt(trend.days)} jours de mesures sur ${fmtInt(trend.span_days)} jours de suivi. C'est trop court pour dire si la température a changé (il faut au moins ${MIN_SPAN_DAYS} jours de suivi, dont 40 avec des mesures). Les points sont déjà tracés.`,
      ),
    ];
  }
  const verdict = verdictOf(trend.delta);
  const now = trend.baseline + trend.delta;
  const from = trend.baseline_from ? fmtDay(trend.baseline_from) : "le début";
  const to = trend.baseline_to ? fmtDay(trend.baseline_to) : "";
  const outcome: Record<Verdict, string> = {
    stable: `une différence de ${fmtTemp(Math.abs(trend.delta))} : c'est stable.`,
    warmer: `${fmtTemp(Math.abs(trend.delta))} de plus qu'au début.`,
    cooler: `${fmtTemp(Math.abs(trend.delta))} de moins qu'au début : c'est plus frais.`,
    unknown: "",
  };
  const lines = [
    h("p", { class: "story" }, `Au début (du ${from} au ${to}), sa température habituelle était de `, h("strong", {}, fmtTemp(trend.baseline)), ". Ces 30 derniers jours : ", h("strong", {}, fmtTemp(now)), `, soit ${outcome[verdict]}`),
  ];
  if (verdict === "warmer") {
    lines.push(h("p", { class: "advice" }, "Si d'autres composants ont aussi chauffé, la chaleur de la saison peut l'expliquer. Si seul celui-ci chauffe, pensez à la poussière, à la pâte thermique ou à un ventilateur."));
  }
  return lines;
}

function legendItem(label: string, className: string): HTMLElement {
  return h("li", {}, h("span", { class: `key ${className}`, "aria-hidden": "true" }), label);
}

/** The opened sensor: its sentence, its curve, its figures. Drawn once it is in the page. */
function detail(component: Component, row: Summary): { element: HTMLElement; draw: () => void } {
  const trend = trendOf(row.hardware, row.sensor);
  const name = sensorLabel(row.sensor, component.kind);
  const element = h("div", { class: "detail" });
  const load = LOADS.find((l) => l.id === state.load);
  element.append(h("h3", {}, name.main), h("p", { class: "muted" }, `${component.title}${component.subtitle ? ` · ${component.subtitle}` : ""} · ${load?.label.toLowerCase()}${name.tech ? ` · capteur « ${name.tech} »` : ""}`));

  if (trend === undefined) {
    element.append(h("p", { class: "muted" }, "Chargement…"));
    return { element, draw: () => {} };
  }
  if (trend === null) {
    element.append(h("p", { class: "story" }, "Impossible de charger la courbe de ce capteur."));
    return { element, draw: () => {} };
  }
  element.append(...story(trend));
  if (trend.points.length === 0) return { element, draw: () => {} };

  const chart = h("div", { class: "chart", role: "img", "aria-label": `Courbe de la température de ${name.main} dans le temps` });
  element.append(chart);
  const legend = h("ul", { class: "legend" }, legendItem("ligne pleine = température habituelle sur 30 jours", "key-line"));
  if (trend.sufficient) legend.append(legendItem("tirets = niveau du début", "key-dash"));
  element.append(legend, h("p", { class: "hint" }, "Glissez sur la courbe pour zoomer, double-cliquez pour revenir. Les bandes grises alternées marquent les mois."));
  element.append(h("details", {}, h("summary", {}, "Voir les chiffres, mois par mois"), monthlyTable(trend)));
  if (trend.sufficient && trend.slope_per_month !== null) {
    element.append(
      h(
        "details",
        {},
        h("summary", {}, "Détails techniques"),
        h(
          "dl",
          { class: "tech-list" },
          h("dt", {}, "Pente générale"), h("dd", {}, `${fmtSigned(trend.slope_per_month, "°C par mois")} (droite des moindres carrés sur toutes les journées)`),
          h("dt", {}, "Jours avec mesures"), h("dd", {}, `${fmtInt(trend.days)} sur ${fmtInt(trend.span_days)} jours de suivi`),
          h("dt", {}, "Niveau du début"), h("dd", {}, "médiane des 30 premiers jours avec mesures"),
          h("dt", {}, "Température récente"), h("dd", {}, "médiane des 30 derniers jours calendaires"),
        ),
      ),
    );
  }
  const index = state.components.indexOf(component);
  const rolling = new Map(trend.rolling.map((r) => [r.day, r.value]));
  const draw = (): void =>
    trackChart(
      drawChart(chart, {
        series: [{ id: row.sensor, label: name.main, color: colorValue(index), values: rolling }],
        unit: "temp",
        reference: trend.baseline === null ? null : { value: trend.baseline, label: `Au début : ${fmtTemp(trend.baseline)}` },
      }),
    );
  return { element, draw };
}

function sensorRow(row: Summary, component: Component, actions: Actions): HTMLElement {
  const selected = isSelected(row);
  const name = sensorLabel(row.sensor, component.kind);
  const tr = h("tr", { tabindex: "0", "aria-expanded": String(selected), class: selected ? "selected" : undefined });
  tr.append(
    h("th", { scope: "row" }, h("span", { class: "sensor" }, name.main), name.tech ? h("span", { class: "tech" }, name.tech) : null),
    h("td", { class: "num" }, row.sufficient && row.baseline !== null ? fmtTemp(row.baseline) : "—"),
    h("td", { class: "num" }, row.current === null ? "—" : fmtTemp(row.current)),
    h("td", {}, row.sufficient ? verdictChip(row) : h("span", { class: "chip chip-unknown" }, `Pas assez de recul (${fmtInt(row.days)} j de mesures)`)),
    h("td", { class: "spark" }, sparkline(row.sparkline, verdictOf(row.sufficient ? row.delta : null))),
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

export function renderDetails(host: HTMLElement, actions: Actions): void {
  clear(host);
  const card = h("section", { id: "components", class: "card" }, h("h2", {}, "Détail de chaque capteur"));
  host.append(card);
  card.append(loadFilter(actions));
  if (state.components.length === 0) {
    card.append(h("p", { class: "muted" }, "Rien à afficher pour ce niveau de charge pour l'instant."));
    return;
  }
  card.append(h("p", { class: "muted" }, "Cliquez sur une ligne pour lire l'histoire du capteur et voir sa courbe."));

  let draw: () => void = () => {};
  for (const component of state.components) {
    const body = h("tbody");
    for (const row of component.rows) {
      body.append(sensorRow(row, component, actions));
      if (isSelected(row)) {
        const d = detail(component, row);
        draw = d.draw;
        body.append(h("tr", { class: "detail-row" }, h("td", { colspan: "5" }, d.element)));
      }
    }
    card.append(
      h(
        "div",
        { class: "table-wrap" },
        h(
          "table",
          {},
          h("caption", {}, component.title, component.subtitle ? h("span", { class: "caption-sub" }, ` — ${component.subtitle}`) : null),
          h("thead", {}, h("tr", {}, h("th", { scope: "col" }, "Capteur"), h("th", { scope: "col", class: "num" }, "Au début"), h("th", { scope: "col", class: "num" }, "Ces 30 derniers jours"), h("th", { scope: "col" }, "Évolution"), h("th", { scope: "col", class: "spark" }, "Courbe"))),
          body,
        ),
      ),
    );
  }
  draw();
}
