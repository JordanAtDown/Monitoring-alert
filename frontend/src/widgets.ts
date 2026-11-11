// Small pieces shared by several pages. They only lay out what the backend wrote.

import type { Cause, Overview, Verdict } from "./api";
import { h, svg } from "./dom";
import type { Actions } from "./state";

/** The word for how something changed since the beginning, with the figure. */
export function verdictChip(v: Verdict): HTMLElement {
  const chip = h("span", { class: `chip chip-${v.id}` }, v.label);
  if (v.detail) chip.append(h("span", { class: "chip-delta" }, v.detail));
  return chip;
}

/**
 * What a change is put down to. Left out when nothing moved: the verdict already says "Stable".
 * The wording is the backend's; the page only decides it deserves a line of its own.
 */
export function causeNote(cause: Cause | null): HTMLElement | null {
  if (!cause || cause.id === "nothing") return null;
  const note = h("span", { class: `comp-cause cause-${cause.id}` }, cause.label);
  if (cause.detail) note.append(h("span", { class: "cause-detail" }, cause.detail));
  return note;
}

/** The little curve takes the color of the trend: warm when it rose, cool when it fell, gray otherwise. */
const SPARK_COLOR: Record<Verdict["id"], string> = {
  warmer: "var(--diverge-warm)",
  cooler: "var(--diverge-cool)",
  stable: "var(--ink-muted)",
  unknown: "var(--ink-muted)",
};

export function sparkline(values: number[], verdict: Verdict["id"]): SVGElement {
  const w = 96;
  const hgt = 26;
  const root = svg("svg", { width: String(w), height: String(hgt), viewBox: `0 0 ${w} ${hgt}`, role: "img", "aria-label": "Évolution sur la période" });
  if (values.length < 2) return root;
  const lo = Math.min(...values);
  const hi = Math.max(...values);
  const span = hi - lo || 1;
  const points = values
    .map((v, i) => `${((i / (values.length - 1)) * (w - 4) + 2).toFixed(1)},${(hgt - 3 - ((v - lo) / span) * (hgt - 6)).toFixed(1)}`)
    .join(" ");
  root.append(svg("polyline", { points, fill: "none", stroke: SPARK_COLOR[verdict], "stroke-width": "2", "stroke-linejoin": "round", "stroke-linecap": "round" }));
  return root;
}

/** "Le PC est… au repos / charge légère…": the filter sits right where the figures it changes are. */
export function loadFilter(load: Overview["load"], actions: Actions): HTMLElement {
  const group = h("div", { class: "segmented", role: "group", "aria-label": "Le PC est…" });
  for (const choice of load.choices) {
    const button = h("button", { type: "button", class: "segment", "aria-pressed": String(choice.id === load.id) }, choice.label);
    button.addEventListener("click", () => {
      if (choice.id !== load.id) actions.setLoad(choice.id);
    });
    group.append(button);
  }
  return h("div", { class: "load-filter" }, h("span", { class: "load-label" }, "Le PC est…"), group, h("p", { class: "filters-hint" }, load.note));
}

/** A table of text: the table view of a chart. */
export function textTable(heads: string[], rows: string[][]): HTMLElement {
  const body = h("tbody");
  for (const row of rows) {
    body.append(h("tr", {}, ...row.map((cell, i) => (i === 0 ? h("th", { scope: "row" }, cell) : h("td", { class: "num" }, cell)))));
  }
  return h("table", { class: "monthly" }, h("thead", {}, h("tr", {}, ...heads.map((t, i) => h("th", { scope: "col", class: i > 0 ? "num" : undefined }, t)))), body);
}
