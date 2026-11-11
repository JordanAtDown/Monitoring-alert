import "./style.css";
import { api, type Load, type Status, type Summary, type Trend } from "./api";
import { drawTrendChart } from "./chart";
import { clear, h, svg } from "./dom";
import {
  fmtAgo, fmtBytes, fmtDay, fmtInt, fmtLongDay, fmtMonthLong, fmtSigned, fmtTemp, parseDay,
} from "./format";
import { readTheme, withAlpha } from "./theme";

const LOADS: { id: Load; label: string; hint: string }[] = [
  { id: "idle", label: "Repos", hint: "Charge inférieure à 15 %" },
  { id: "light", label: "Léger", hint: "Charge de 15 à 40 %" },
  { id: "moderate", label: "Modéré", hint: "Charge de 40 à 75 %" },
  { id: "heavy", label: "Intense", hint: "Charge supérieure à 75 %" },
];
const MIN_SPAN_DAYS = 60;
const STATUS_REFRESH_MS = 30_000;
const DATA_REFRESH_MS = 10 * 60_000;

interface Selection {
  hardware: string;
  sensor: string;
}

const state = {
  load: "idle" as Load,
  selected: null as Selection | null,
  status: null as Status | null,
  summary: [] as Summary[],
  trend: null as Trend | null,
  disposeChart: (() => {}) as () => void,
};

// ── URL hash: #load=idle&hw=…&sensor=… ───────────────────────────────────────

function readHash(): void {
  const params = new URLSearchParams(location.hash.slice(1));
  const load = params.get("load");
  if (LOADS.some((l) => l.id === load)) state.load = load as Load;
  const hardware = params.get("hw");
  const sensor = params.get("sensor");
  state.selected = hardware && sensor ? { hardware, sensor } : null;
}

function writeHash(): void {
  const params = new URLSearchParams({ load: state.load });
  if (state.selected) {
    params.set("hw", state.selected.hardware);
    params.set("sensor", state.selected.sensor);
  }
  history.replaceState(null, "", `#${params.toString()}`);
}

// ── Header: collector health ─────────────────────────────────────────────────

function renderStatus(): void {
  const host = document.getElementById("status") as HTMLElement;
  clear(host);
  const s = state.status;
  if (!s) {
    host.append(h("p", { class: "status status-bad" }, h("span", { class: "status-icon", "aria-hidden": "true" }, "✕"), "Le serveur ne répond pas"));
    return;
  }
  const c = s.collector;
  const stale = c.last_success === null
    ? true
    : (new Date(s.now).getTime() - new Date(c.last_success).getTime()) / 1000 > c.interval_secs * 3;

  let kind = "ok";
  let icon = "✓";
  let text = `Collecte active — dernière mesure ${fmtAgo(c.last_success, s.now)} · ${fmtInt(c.last_sensor_count)} capteurs`;
  if (c.consecutive_failures > 0) {
    kind = "bad";
    icon = "✕";
    text = `LibreHardwareMonitor injoignable depuis ${fmtAgo(c.last_success, s.now).replace("il y a ", "")} (${fmtInt(c.consecutive_failures)} échecs)`;
  } else if (stale) {
    kind = "warn";
    icon = "⚠";
    text = "En attente de la première mesure…";
  }
  host.append(h("p", { class: `status status-${kind}` }, h("span", { class: "status-icon", "aria-hidden": "true" }, icon), text));
  if (c.consecutive_failures > 0 && c.last_error) {
    host.append(h("p", { class: "status-detail" }, c.last_error));
  }
  const since = s.db.first_ts ? ` · mesures brutes depuis le ${fmtDay(s.db.first_ts.slice(0, 10))}` : "";
  host.append(
    h(
      "p",
      { class: "status-detail" },
      `Base ${fmtBytes(s.db.size_bytes)} · ${fmtInt(s.db.total_snapshots)} relevés bruts conservés ${s.db.raw_retention_days} j${since} · v${s.version}`,
    ),
  );
}

// ── Load selector ────────────────────────────────────────────────────────────

function renderFilters(): void {
  const host = document.getElementById("filters") as HTMLElement;
  clear(host);
  host.append(h("span", { class: "filters-label", id: "load-label" }, "Charge de la machine"));
  const group = h("div", { class: "segmented", role: "group", "aria-labelledby": "load-label" });
  for (const load of LOADS) {
    const button = h(
      "button",
      { type: "button", class: "segment", "aria-pressed": String(load.id === state.load), title: load.hint },
      load.label,
    );
    button.addEventListener("click", () => {
      if (state.load === load.id) return;
      state.load = load.id;
      state.selected = null;
      void refreshData();
    });
    group.append(button);
  }
  host.append(group);
}

// ── Overview table ───────────────────────────────────────────────────────────

function sparkline(values: number[]): SVGElement {
  const w = 96;
  const hgt = 26;
  const root = svg("svg", { width: String(w), height: String(hgt), viewBox: `0 0 ${w} ${hgt}`, role: "img", "aria-label": "Tendance de la médiane glissante" });
  if (values.length < 2) return root;
  const lo = Math.min(...values);
  const hi = Math.max(...values);
  const span = hi - lo || 1;
  const points = values
    .map((v, i) => `${((i / (values.length - 1)) * (w - 4) + 2).toFixed(1)},${(hgt - 3 - ((v - lo) / span) * (hgt - 6)).toFixed(1)}`)
    .join(" ");
  root.append(svg("polyline", { points, fill: "none", stroke: "var(--series-1)", "stroke-width": "2", "stroke-linejoin": "round", "stroke-linecap": "round" }));
  return root;
}

function latestValue(hardware: string, sensor: string): number | null {
  const reading = state.status?.latest?.readings.find((r) => r.hardware === hardware && r.sensor === sensor);
  return reading ? reading.value : null;
}

function deltaCell(delta: number | null): HTMLElement {
  const cell = h("td", { class: "num delta" });
  if (delta === null) {
    cell.append("—");
    return cell;
  }
  const theme = readTheme();
  const strength = Math.min(Math.abs(delta) / 6, 1);
  cell.style.background = withAlpha(delta >= 0 ? theme.warm : theme.cool, 0.08 + 0.3 * strength);
  cell.append(h("span", { class: "arrow", "aria-hidden": "true" }, delta > 0.05 ? "▲" : delta < -0.05 ? "▼" : "▶"), fmtSigned(delta));
  return cell;
}

function renderSummary(): void {
  const host = document.getElementById("summary") as HTMLElement;
  clear(host);
  if (state.summary.length === 0) {
    host.append(
      h("p", { class: "empty" }, "Aucune statistique quotidienne pour cette charge pour l'instant. Les agrégats apparaissent le lendemain de la première collecte, et un capteur n'a de point que s'il a eu au moins 30 minutes de mesures ce jour-là."),
    );
    return;
  }
  const head = h(
    "tr",
    {},
    ...["Capteur", "Maintenant", "Référence", "Médiane 30 j", "Écart", "Pente", "Tendance", "Recul"].map((t, i) =>
      h("th", { scope: "col", class: i >= 1 && i <= 5 ? "num" : undefined }, t),
    ),
  );
  const body = h("tbody");
  for (const row of state.summary) {
    const selected = state.selected?.hardware === row.hardware && state.selected?.sensor === row.sensor;
    const tr = h("tr", { tabindex: "0", "aria-selected": String(selected), class: selected ? "selected" : undefined });
    const now = latestValue(row.hardware, row.sensor);
    tr.append(
      h("th", { scope: "row" }, h("span", { class: "sensor" }, row.sensor), h("span", { class: "hardware" }, row.hardware)),
      h("td", { class: "num" }, now === null ? "—" : fmtTemp(now)),
      h("td", { class: "num" }, row.baseline === null ? "—" : fmtTemp(row.baseline)),
      h("td", { class: "num" }, row.current === null ? "—" : fmtTemp(row.current)),
      deltaCell(row.delta),
      h("td", { class: "num" }, row.slope_per_month === null ? "—" : fmtSigned(row.slope_per_month, "°C/mois")),
      h("td", { class: "spark" }, sparkline(row.sparkline)),
      h("td", { class: row.sufficient ? "" : "short" }, row.sufficient ? `${fmtInt(row.span_days)} j` : `${fmtInt(row.span_days)} / ${MIN_SPAN_DAYS} j`),
    );
    const select = (): void => {
      state.selected = { hardware: row.hardware, sensor: row.sensor };
      writeHash();
      renderSummary();
      void refreshTrend();
    };
    tr.addEventListener("click", select);
    tr.addEventListener("keydown", (e) => {
      if (e.key === "Enter" || e.key === " ") {
        e.preventDefault();
        select();
      }
    });
    body.append(tr);
  }
  host.append(h("table", {}, h("thead", {}, head), body));
}

// ── Chart panel ──────────────────────────────────────────────────────────────

function stat(label: string, value: string, note?: string): HTMLElement {
  return h("div", { class: "stat" }, h("dt", {}, label), h("dd", {}, value), note ? h("p", {}, note) : null);
}

/** Monthly digest of the daily points: the table view of the chart. */
function monthlyTable(trend: Trend): HTMLElement {
  const months = new Map<number, { medians: number[]; min: number; max: number }>();
  for (const p of trend.points) {
    const d = new Date(parseDay(p.day) * 1000);
    const key = Date.UTC(d.getUTCFullYear(), d.getUTCMonth(), 1) / 1000;
    const entry = months.get(key) ?? { medians: [], min: Infinity, max: -Infinity };
    entry.medians.push(p.median);
    entry.min = Math.min(entry.min, p.min);
    entry.max = Math.max(entry.max, p.max);
    months.set(key, entry);
  }
  const body = h("tbody");
  for (const [key, m] of [...months].sort((a, b) => a[0] - b[0])) {
    const sorted = [...m.medians].sort((a, b) => a - b);
    const mid = sorted.length >> 1;
    const median = sorted.length % 2 ? sorted[mid] : (sorted[mid - 1] + sorted[mid]) / 2;
    body.append(
      h("tr", {}, h("th", { scope: "row" }, fmtMonthLong(key)), h("td", { class: "num" }, fmtInt(m.medians.length)), h("td", { class: "num" }, fmtTemp(median)), h("td", { class: "num" }, fmtTemp(m.min)), h("td", { class: "num" }, fmtTemp(m.max))),
    );
  }
  return h(
    "table",
    { class: "monthly" },
    h("thead", {}, h("tr", {}, ...["Mois", "Jours", "Médiane des jours", "Min", "Max"].map((t, i) => h("th", { scope: "col", class: i > 0 ? "num" : undefined }, t)))),
    body,
  );
}

function legendItem(label: string, className: string): HTMLElement {
  return h("li", {}, h("span", { class: `key ${className}`, "aria-hidden": "true" }), label);
}

function renderChartPanel(): void {
  state.disposeChart();
  state.disposeChart = () => {};
  const host = document.getElementById("detail") as HTMLElement;
  clear(host);
  const sel = state.selected;
  const trend = state.trend;
  if (!sel || !trend) return;
  const load = LOADS.find((l) => l.id === state.load);

  host.append(
    h("h2", {}, sel.sensor, h("span", { class: "hardware" }, ` — ${sel.hardware} · charge « ${load?.label ?? state.load} »`)),
  );

  if (trend.points.length === 0) {
    host.append(h("p", { class: "empty" }, "Pas encore de jour complet avec assez de mesures pour ce capteur à cette charge."));
    return;
  }

  if (!trend.sufficient) {
    host.append(
      h(
        "p",
        { class: "notice" },
        `Pas encore assez de recul pour parler de dérive : ${fmtInt(trend.span_days)} jours de suivi (${fmtInt(trend.days)} avec des mesures), il en faut ${MIN_SPAN_DAYS} sur la durée et 40 avec des mesures. Les médianes quotidiennes sont déjà tracées.`,
      ),
    );
  } else if (trend.baseline !== null && trend.delta !== null && trend.slope_per_month !== null) {
    const stats = h("dl", { class: "stats" });
    stats.append(
      stat("Référence", fmtTemp(trend.baseline), trend.baseline_from && trend.baseline_to ? `médiane du ${fmtDay(trend.baseline_from)} au ${fmtDay(trend.baseline_to)}` : undefined),
      stat("Médiane 30 j", fmtTemp(trend.baseline + trend.delta)),
      stat("Écart à la référence", fmtSigned(trend.delta)),
      stat("Pente", fmtSigned(trend.slope_per_month, "°C/mois"), "droite de tendance sur toute la durée"),
    );
    host.append(stats);
  }

  const chart = h("div", { class: "chart", "aria-label": `Courbe de dérive de ${sel.sensor}` });
  host.append(chart);
  state.disposeChart = drawTrendChart(chart, trend);

  const legend = h("ul", { class: "legend" }, legendItem("Médiane quotidienne (la bande claire va du min au p95)", "key-dots"), legendItem("Médiane glissante 30 j", "key-line"));
  if (trend.sufficient) {
    legend.append(legendItem("Référence", "key-dash"), legendItem("Tendance", "key-dots-fine"));
  }
  host.append(legend);
  host.append(
    h("p", { class: "hint" }, "Glisser sur le graphique pour zoomer, double-clic pour revenir. Les mois en alternance claire/foncée aident à reconnaître l'effet des saisons : sans capteur de température ambiante, un été chaud peut ressembler à une dérive."),
  );
  host.append(h("details", {}, h("summary", {}, "Vue tableau (par mois)"), monthlyTable(trend)));
  void fmtLongDay;
}

// ── Data flow ────────────────────────────────────────────────────────────────

async function refreshStatus(): Promise<void> {
  try {
    state.status = await api.status();
  } catch {
    state.status = null;
  }
  renderStatus();
  if (state.summary.length > 0) renderSummary();
}

async function refreshTrend(): Promise<void> {
  const sel = state.selected;
  const panel = document.getElementById("detail") as HTMLElement;
  if (!sel) {
    state.trend = null;
    renderChartPanel();
    return;
  }
  panel.classList.add("loading");
  try {
    state.trend = await api.trend(sel.hardware, sel.sensor, state.load);
  } catch {
    state.trend = null;
  }
  panel.classList.remove("loading");
  renderChartPanel();
}

async function refreshData(): Promise<void> {
  renderFilters();
  try {
    state.summary = await api.summary(state.load);
  } catch {
    state.summary = [];
  }
  const known = state.selected && state.summary.some((s) => s.hardware === state.selected?.hardware && s.sensor === state.selected?.sensor);
  if (!known) {
    const first = state.summary.find((s) => s.sufficient) ?? state.summary[0];
    state.selected = first ? { hardware: first.hardware, sensor: first.sensor } : null;
  }
  writeHash();
  renderSummary();
  await refreshTrend();
}

function mount(): void {
  const app = document.getElementById("app") as HTMLElement;
  app.append(
    h("header", {}, h("h1", {}, "Monitoring Alert"), h("p", { class: "tagline" }, "Évolution des températures dans le temps"), h("div", { id: "status", "aria-live": "polite" })),
    h("section", { id: "filters", class: "filters", "aria-label": "Filtres" }),
    h(
      "main",
      {},
      h("section", { "aria-labelledby": "summary-title" }, h("h2", { id: "summary-title" }, "Tous les capteurs, du plus fort écart au plus faible"), h("div", { id: "summary", class: "table-wrap" })),
      h("section", { id: "detail", class: "detail", "aria-live": "polite" }),
    ),
  );
}

function start(): void {
  mount();
  readHash();
  void refreshStatus().then(refreshData);
  setInterval(() => void refreshStatus(), STATUS_REFRESH_MS);
  setInterval(() => void refreshData(), DATA_REFRESH_MS);
  addEventListener("hashchange", () => {
    readHash();
    void refreshData();
  });
  // Canvas colors are read at draw time: redraw when the OS theme flips.
  matchMedia("(prefers-color-scheme: dark)").addEventListener("change", () => {
    renderSummary();
    renderChartPanel();
  });
}

start();
