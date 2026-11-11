// Small pieces shared by several pages.

import type { Summary, Trend } from "./api";
import type { Component } from "./components";
import { h, svg } from "./dom";
import { fmtInt, fmtMonthLong, fmtSigned, fmtTemp, parseDay } from "./format";
import { state, type Actions } from "./state";
import { LOADS, verdictOf, VERDICT_LABEL, type Verdict } from "./text";

/** The word for how a sensor changed since the beginning, with the figure. */
export function verdictChip(summary: Summary): HTMLElement {
  const verdict = verdictOf(summary.sufficient ? summary.delta : null);
  const chip = h("span", { class: `chip chip-${verdict}` }, VERDICT_LABEL[verdict]);
  if (summary.sufficient && summary.delta !== null) chip.append(h("span", { class: "chip-delta" }, fmtSigned(summary.delta)));
  return chip;
}

/** The verdict of a whole component is the one of the sensor that stands for it. */
export function componentVerdict(c: Component): Verdict {
  return verdictOf(c.main.sufficient ? c.main.delta : null);
}

const SPARK_COLOR: Record<Verdict, string> = {
  warmer: "var(--diverge-warm)",
  cooler: "var(--diverge-cool)",
  stable: "var(--ink-muted)",
  unknown: "var(--ink-muted)",
};

/** The little curve takes the color of the trend: warm when it rose, cool when it fell, gray when stable. */
export function sparkline(values: number[], verdict: Verdict): SVGElement {
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

function median(values: number[]): number {
  const sorted = [...values].sort((a, b) => a - b);
  const mid = sorted.length >> 1;
  return sorted.length % 2 ? sorted[mid] : (sorted[mid - 1] + sorted[mid]) / 2;
}

function monthOf(day: string): number {
  const d = new Date(parseDay(day) * 1000);
  return Date.UTC(d.getUTCFullYear(), d.getUTCMonth(), 1) / 1000;
}

/** One sensor, month by month: the table view of its chart. */
export function monthlyTable(trend: Trend): HTMLElement {
  const months = new Map<number, { medians: number[]; min: number; max: number }>();
  for (const p of trend.points) {
    const key = monthOf(p.day);
    const entry = months.get(key) ?? { medians: [], min: Infinity, max: -Infinity };
    entry.medians.push(p.median);
    entry.min = Math.min(entry.min, p.min);
    entry.max = Math.max(entry.max, p.max);
    months.set(key, entry);
  }
  const body = h("tbody");
  for (const [key, m] of [...months].sort((a, b) => a[0] - b[0])) {
    body.append(
      h("tr", {}, h("th", { scope: "row" }, fmtMonthLong(key)), h("td", { class: "num" }, fmtInt(m.medians.length)), h("td", { class: "num" }, fmtTemp(median(m.medians))), h("td", { class: "num" }, fmtTemp(m.min)), h("td", { class: "num" }, fmtTemp(m.max))),
    );
  }
  const heads = ["Mois", "Jours mesurés", "Température habituelle", "La plus basse", "La plus haute"];
  return h("table", { class: "monthly" }, h("thead", {}, h("tr", {}, ...heads.map((t, i) => h("th", { scope: "col", class: i > 0 ? "num" : undefined }, t)))), body);
}

/** Several components, month by month: the table view of the curves. */
export function monthlyMatrix(columns: { label: string; trend: Trend; baseline: number | null }[], delta: boolean): HTMLElement {
  const perColumn = columns.map(({ trend, baseline }) => {
    const byMonth = new Map<number, number[]>();
    for (const p of trend.points) byMonth.set(monthOf(p.day), [...(byMonth.get(monthOf(p.day)) ?? []), p.median]);
    return new Map([...byMonth].map(([m, v]) => [m, median(v) - (delta ? (baseline ?? 0) : 0)]));
  });
  const months = [...new Set(perColumn.flatMap((m) => [...m.keys()]))].sort((a, b) => a - b);
  const body = h("tbody");
  for (const month of months) {
    body.append(
      h("tr", {}, h("th", { scope: "row" }, fmtMonthLong(month)), ...perColumn.map((m) => h("td", { class: "num" }, m.has(month) ? (delta ? fmtSigned(m.get(month) as number) : fmtTemp(m.get(month) as number)) : "—"))),
    );
  }
  return h("table", { class: "monthly" }, h("thead", {}, h("tr", {}, h("th", { scope: "col" }, "Mois"), ...columns.map((c) => h("th", { scope: "col", class: "num" }, c.label)))), body);
}

export function plural(n: number, one: string, many: string): string {
  return n > 1 ? many : one;
}

/** "Le PC est… au repos / charge légère…": the filter sits right where the figures it changes are. */
export function loadFilter(actions: Actions): HTMLElement {
  const group = h("div", { class: "segmented", role: "group", "aria-label": "Le PC est…" });
  for (const load of LOADS) {
    const button = h("button", { type: "button", class: "segment", "aria-pressed": String(load.id === state.load) }, load.label);
    button.addEventListener("click", () => {
      if (state.load !== load.id) actions.setLoad(load.id);
    });
    group.append(button);
  }
  const current = LOADS.find((l) => l.id === state.load);
  return h(
    "div",
    { class: "load-filter" },
    h("span", { class: "load-label" }, "Le PC est…"),
    group,
    h("p", { class: "filters-hint" }, `${current?.label} : ${current?.hint}. On compare toujours des mesures prises dans les mêmes conditions.`),
  );
}
