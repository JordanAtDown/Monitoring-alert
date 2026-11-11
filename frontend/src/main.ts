import "./style.css";
import { api, type Load, type Status, type Summary, type Trend, type UpdateInfo } from "./api";
import { drawTrendChart } from "./chart";
import { clear, h, svg } from "./dom";
import { fmtAgo, fmtBytes, fmtDay, fmtInt, fmtMonthLong, fmtSigned, fmtTemp, parseDay } from "./format";
import {
  groupTitle, hardwareKind, KIND_ORDER, LOADS, MIN_SPAN_DAYS, sensorLabel, spanInWords, STABLE_BAND,
  verdictOf, VERDICT_LABEL, type Kind, type Verdict,
} from "./text";
import { fillUpdateDialog, renderUpdateButton, versionLine } from "./update";

const STATUS_REFRESH_MS = 30_000;
const DATA_REFRESH_MS = 10 * 60_000;
const UPDATE_REFRESH_MS = 30 * 60_000;

interface Selection {
  hardware: string;
  sensor: string;
}

const state = {
  load: "idle" as Load,
  selected: null as Selection | null,
  status: null as Status | null,
  update: null as UpdateInfo | null,
  summary: [] as Summary[],
  trend: null as Trend | null,
  showBand: false,
  showTrendLine: false,
  disposeChart: (() => {}) as () => void,
};

const $ = (id: string): HTMLElement => document.getElementById(id) as HTMLElement;

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

/** The help stays open until the reader closes it once. Storage may be unavailable. */
function helpWasClosed(): boolean {
  try {
    return localStorage.getItem("help-closed") === "1";
  } catch {
    return false;
  }
}
function rememberHelp(closed: boolean): void {
  try {
    localStorage.setItem("help-closed", closed ? "1" : "0");
  } catch {
    /* private window: the help simply opens again next time */
  }
}

// ── Collection status ────────────────────────────────────────────────────────

function renderStatus(): void {
  const host = $("status");
  clear(host);
  const s = state.status;
  const line = (kind: "ok" | "warn" | "bad", icon: string, text: string, detail?: string): void => {
    host.append(
      h("p", { class: `status status-${kind}` }, h("span", { class: "status-icon", "aria-hidden": "true" }, icon), text),
    );
    if (detail) host.append(h("p", { class: "status-detail" }, detail));
  };
  if (!s) {
    line("bad", "✕", "La page ne parvient plus à joindre le programme.", "Il est peut-être arrêté : lancez Monitoring depuis le menu Démarrer.");
    return;
  }
  const c = s.collector;
  const stale = c.last_success === null
    ? true
    : (new Date(s.now).getTime() - new Date(c.last_success).getTime()) / 1000 > c.interval_secs * 3;
  if (c.consecutive_failures > 0) {
    const since = c.last_success ? ` depuis ${fmtAgo(c.last_success, s.now).replace("il y a ", "")}` : "";
    line(
      "bad", "✕",
      `Les températures ne sont plus relevées${since}.`,
      "LibreHardwareMonitor ne répond pas : lancez-le en administrateur et vérifiez que « Remote Web Server » est activé (Options).",
    );
  } else if (stale) {
    line("warn", "⚠", "En attente de la première mesure…");
  } else {
    line("ok", "✓", `Tout fonctionne : dernière mesure ${fmtAgo(c.last_success, s.now)} (${fmtInt(c.last_sensor_count)} capteurs).`);
  }
}

// ── Choice of the load level ─────────────────────────────────────────────────

function renderFilters(): void {
  const host = $("filters");
  clear(host);
  host.append(h("h2", { id: "load-title" }, "Le PC est…"));
  const group = h("div", { class: "segmented", role: "group", "aria-labelledby": "load-title" });
  for (const load of LOADS) {
    const button = h(
      "button",
      { type: "button", class: "segment", "aria-pressed": String(load.id === state.load) },
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
  const current = LOADS.find((l) => l.id === state.load);
  host.append(
    h("p", { class: "filters-hint" }, `${current?.label} : ${current?.hint}. On compare toujours des mesures prises dans les mêmes conditions.`),
  );
}

// ── The answer, in a few words ───────────────────────────────────────────────

function plural(n: number, one: string, many: string): string {
  return n > 1 ? many : one;
}

function countTile(verdict: Verdict, n: number): HTMLElement {
  const words: Record<Verdict, [string, string]> = {
    warmer: ["composant plus chaud qu'au début", "composants plus chauds qu'au début"],
    stable: ["composant stable", "composants stables"],
    cooler: ["composant plus froid qu'au début", "composants plus froids qu'au début"],
    unknown: ["composant sans assez de recul", "composants sans assez de recul"],
  };
  return h(
    "div",
    { class: `tile tile-${verdict}` },
    h("span", { class: "tile-number" }, fmtInt(n)),
    h("span", { class: "tile-words" }, plural(n, words[verdict][0], words[verdict][1])),
  );
}

function renderVerdict(): void {
  const host = $("verdict");
  clear(host);
  host.append(h("h2", {}, "En résumé"));
  const load = LOADS.find((l) => l.id === state.load);
  const known = state.summary.filter((s) => s.sufficient && s.delta !== null);

  if (state.summary.length === 0) {
    host.append(
      h("p", { class: "lead" }, "Aucune mesure résumée pour l'instant."),
      h("p", { class: "muted" }, "Les premiers résumés apparaissent le lendemain de la première collecte."),
    );
    return;
  }
  if (known.length === 0) {
    const days = Math.max(...state.summary.map((s) => s.days));
    host.append(
      h("p", { class: "lead" }, "Pas encore assez d'historique pour comparer."),
      h(
        "p",
        { class: "muted" },
        `Pour dire si une température a changé, il faut au moins ${MIN_SPAN_DAYS} jours de suivi dont 40 jours avec des mesures ${load?.when}. Pour l'instant : ${fmtInt(days)} jour${days > 1 ? "s" : ""} de mesures.`,
      ),
    );
    return;
  }

  const span = Math.max(...known.map((s) => s.span_days));
  const since = state.status?.db.history_since;
  host.append(
    h(
      "p",
      { class: "lead" },
      `${since ? `Depuis le ${fmtDay(since)}` : "Depuis le début"} (${spanInWords(span)} de suivi), ${load?.when} :`,
    ),
  );
  const counts: Record<Verdict, number> = { warmer: 0, stable: 0, cooler: 0, unknown: 0 };
  for (const s of state.summary) counts[verdictOf(s.sufficient ? s.delta : null)] += 1;
  const tiles = h("div", { class: "tiles" }, countTile("warmer", counts.warmer), countTile("stable", counts.stable), countTile("cooler", counts.cooler));
  if (counts.unknown > 0) tiles.append(countTile("unknown", counts.unknown));
  host.append(tiles);

  const top = [...known].sort((a, b) => Math.abs(b.delta as number) - Math.abs(a.delta as number))[0];
  const topDelta = top.delta as number;
  const label = sensorLabel(top.sensor).main;
  const sentences = [
    Math.abs(topDelta) < STABLE_BAND
      ? "Aucun composant n'a vraiment changé de température."
      : `Le changement le plus marqué : ${label} (${fmtSigned(topDelta)}).`,
    Math.abs(topDelta) < 3
      ? "Les écarts restent faibles (moins de 3 °C)."
      : "Cliquez sur ce composant pour voir sa courbe ; une saison chaude peut expliquer une partie de l'écart.",
  ];
  host.append(h("p", {}, sentences.join(" ")));
}

// ── The components ───────────────────────────────────────────────────────────

function sparkline(values: number[]): SVGElement {
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
  root.append(svg("polyline", { points, fill: "none", stroke: "var(--series-1)", "stroke-width": "2", "stroke-linejoin": "round", "stroke-linecap": "round" }));
  return root;
}

function verdictChip(summary: Summary): HTMLElement {
  const verdict = verdictOf(summary.sufficient ? summary.delta : null);
  const chip = h("span", { class: `chip chip-${verdict}` }, VERDICT_LABEL[verdict]);
  if (summary.sufficient && summary.delta !== null) chip.append(h("span", { class: "chip-delta" }, fmtSigned(summary.delta)));
  return chip;
}

interface Group {
  hardware: string;
  kind: Kind;
  rows: Summary[];
}

function groupSummaries(summaries: Summary[]): Group[] {
  const byHardware = new Map<string, Summary[]>();
  for (const s of summaries) byHardware.set(s.hardware, [...(byHardware.get(s.hardware) ?? []), s]);
  const groups: Group[] = [...byHardware].map(([hardware, rows]) => ({
    hardware,
    kind: hardwareKind(hardware, rows.map((r) => r.sensor)),
    rows: [...rows].sort((a, b) => (b.delta ?? -Infinity) - (a.delta ?? -Infinity)),
  }));
  return groups.sort((a, b) => KIND_ORDER.indexOf(a.kind) - KIND_ORDER.indexOf(b.kind) || a.hardware.localeCompare(b.hardware));
}

function renderComponents(): void {
  const host = $("components");
  clear(host);
  host.append(h("h2", {}, "Vos composants"));
  if (state.summary.length === 0) {
    host.append(h("p", { class: "muted" }, "Rien à afficher pour ce niveau de charge pour l'instant."));
    return;
  }
  host.append(h("p", { class: "muted" }, "Cliquez sur une ligne pour voir la courbe du composant."));

  for (const group of groupSummaries(state.summary)) {
    const body = h("tbody");
    for (const row of group.rows) {
      const selected = state.selected?.hardware === row.hardware && state.selected?.sensor === row.sensor;
      const name = sensorLabel(row.sensor);
      const tr = h("tr", { tabindex: "0", "aria-selected": String(selected), class: selected ? "selected" : undefined });
      tr.append(
        h("th", { scope: "row" }, h("span", { class: "sensor" }, name.main), name.tech ? h("span", { class: "tech" }, name.tech) : null),
        h("td", { class: "num" }, row.sufficient && row.baseline !== null ? fmtTemp(row.baseline) : "—"),
        h("td", { class: "num" }, row.current === null ? "—" : fmtTemp(row.current)),
        h("td", {}, row.sufficient ? verdictChip(row) : h("span", { class: "chip chip-unknown" }, `Pas assez de recul (${fmtInt(row.days)} j de mesures)`)),
        h("td", { class: "spark" }, sparkline(row.sparkline)),
      );
      const select = (): void => {
        state.selected = { hardware: row.hardware, sensor: row.sensor };
        writeHash();
        renderComponents();
        void refreshTrend().then(() => $("detail").scrollIntoView({ behavior: "smooth", block: "start" }));
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
    host.append(
      h(
        "div",
        { class: "table-wrap" },
        h(
          "table",
          {},
          h("caption", {}, groupTitle(group.kind, group.hardware)),
          h("thead", {}, h("tr", {},
            h("th", { scope: "col" }, "Composant"),
            h("th", { scope: "col", class: "num" }, "Au début"),
            h("th", { scope: "col", class: "num" }, "Ces 30 derniers jours"),
            h("th", { scope: "col" }, "Évolution"),
            h("th", { scope: "col", class: "spark" }, "Courbe"),
          )),
          body,
        ),
      ),
    );
  }
}

// ── The chosen component ─────────────────────────────────────────────────────

/** What the numbers mean, as sentences. */
function story(trend: Trend): HTMLElement[] {
  if (trend.points.length === 0) {
    return [h("p", { class: "story" }, "Pas encore de journée complète avec assez de mesures pour ce composant à ce niveau de charge.")];
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
    h(
      "p",
      { class: "story" },
      `Au début (du ${from} au ${to}), sa température habituelle était de `,
      h("strong", {}, fmtTemp(trend.baseline)),
      ". Ces 30 derniers jours : ",
      h("strong", {}, fmtTemp(now)),
      `, soit ${outcome[verdict]}`,
    ),
  ];
  if (verdict === "warmer") {
    lines.push(
      h(
        "p",
        { class: "advice" },
        "Si d'autres composants ont aussi chauffé, la chaleur de la saison peut l'expliquer. Si seul celui-ci chauffe, pensez à la poussière, à la pâte thermique ou à un ventilateur.",
      ),
    );
  }
  return lines;
}

function legendItem(label: string, className: string): HTMLElement {
  return h("li", {}, h("span", { class: `key ${className}`, "aria-hidden": "true" }), label);
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
    h("thead", {}, h("tr", {}, ...["Mois", "Jours mesurés", "Température habituelle", "La plus basse", "La plus haute"].map((t, i) => h("th", { scope: "col", class: i > 0 ? "num" : undefined }, t)))),
    body,
  );
}

function toggle(label: string, checked: boolean, onChange: (value: boolean) => void): HTMLElement {
  const input = h("input", { type: "checkbox" });
  input.checked = checked;
  input.addEventListener("change", () => onChange(input.checked));
  return h("label", { class: "toggle" }, input, ` ${label}`);
}

function renderDetail(): void {
  state.disposeChart();
  state.disposeChart = () => {};
  const host = $("detail");
  clear(host);
  const sel = state.selected;
  const trend = state.trend;
  if (!sel || !trend) {
    host.hidden = true;
    return;
  }
  host.hidden = false;
  const load = LOADS.find((l) => l.id === state.load);
  const name = sensorLabel(sel.sensor);
  const kind = hardwareKind(sel.hardware, state.summary.filter((s) => s.hardware === sel.hardware).map((s) => s.sensor));

  host.append(
    h("h2", {}, name.main),
    h("p", { class: "muted" }, `${groupTitle(kind, sel.hardware)} · ${load?.label.toLowerCase()}${name.tech ? ` · capteur « ${name.tech} »` : ""}`),
    ...story(trend),
  );
  if (trend.points.length === 0) return;

  const chart = h("div", { class: "chart", role: "img", "aria-label": `Courbe de la température de ${name.main} dans le temps` });
  host.append(chart);
  const draw = (): void => {
    state.disposeChart();
    state.disposeChart = drawTrendChart(chart, trend, { band: state.showBand, trendLine: state.showTrendLine && trend.sufficient });
  };
  draw();

  const legend = h("ul", { class: "legend" }, legendItem("un point = un jour", "key-dots"), legendItem("ligne pleine = température habituelle sur 30 jours", "key-line"));
  if (trend.sufficient) legend.append(legendItem("tirets = niveau du début", "key-dash"));
  host.append(legend);

  const toggles = h("div", { class: "toggles" }, toggle("Montrer la plage de chaque jour (de la plus basse à la plus haute)", state.showBand, (v) => { state.showBand = v; draw(); }));
  if (trend.sufficient) toggles.append(toggle("Montrer la tendance générale", state.showTrendLine, (v) => { state.showTrendLine = v; draw(); }));
  host.append(toggles);
  host.append(h("p", { class: "hint" }, "Glissez sur la courbe pour zoomer, double-cliquez pour revenir. Les bandes grises alternées marquent les mois."));

  host.append(h("details", {}, h("summary", {}, "Voir les chiffres, mois par mois"), monthlyTable(trend)));
  if (trend.sufficient && trend.slope_per_month !== null) {
    host.append(
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
}

// ── Where the data is ────────────────────────────────────────────────────────

function renderData(): void {
  const host = $("data");
  clear(host);
  const s = state.status;
  if (!s) {
    host.hidden = true;
    return;
  }
  host.hidden = false;
  const minutes = Math.round(s.collector.interval_secs / 60);
  host.append(
    h("h2", {}, "Vos données"),
    h(
      "ul",
      { class: "data-list" },
      h("li", {}, h("strong", {}, "Résumé de chaque jour"), ` pour tous les capteurs${s.db.history_since ? `, depuis le ${fmtDay(s.db.history_since)}` : ""} : conservé sans limite. C'est lui qui permet de suivre l'évolution sur des mois.`),
      h("li", {}, h("strong", {}, "Mesures détaillées"), ` (${minutes === 1 ? "une par minute" : `une toutes les ${fmtInt(minutes)} minutes`}) : conservées ${fmtInt(s.db.raw_retention_days)} jours, ${fmtInt(s.db.total_snapshots)} relevés actuellement. Avant d'être supprimées, elles sont résumées.`),
      h("li", {}, h("strong", {}, "Fichier"), ` : ${s.db.path} (${fmtBytes(s.db.size_bytes)})`),
    ),
  );
}

function renderFooter(): void {
  const host = $("footer");
  clear(host);
  host.append(h("p", {}, versionLine(state.status?.version ?? null, state.update)));
  const dialog = $("update-dialog") as HTMLDialogElement;
  renderUpdateButton($("update-slot"), state.update, dialog);
  fillUpdateDialog(dialog, state.update);
}

// ── Data flow ────────────────────────────────────────────────────────────────

async function refreshStatus(): Promise<void> {
  try {
    state.status = await api.status();
  } catch {
    state.status = null;
  }
  renderStatus();
  renderData();
  renderFooter();
  if (state.summary.length > 0) renderVerdict();
}

async function refreshUpdate(): Promise<void> {
  try {
    state.update = await api.update();
  } catch {
    state.update = null;
  }
  renderFooter();
}

async function refreshTrend(): Promise<void> {
  const sel = state.selected;
  const panel = $("detail");
  if (!sel) {
    state.trend = null;
    renderDetail();
    return;
  }
  panel.classList.add("loading");
  try {
    state.trend = await api.trend(sel.hardware, sel.sensor, state.load);
  } catch {
    state.trend = null;
  }
  panel.classList.remove("loading");
  renderDetail();
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
  renderVerdict();
  renderComponents();
  await refreshTrend();
}

function helpBlock(): HTMLElement {
  const details = h(
    "details",
    { id: "help", class: "help" },
    h("summary", {}, "Comment lire cette page ?"),
    h(
      "ol",
      {},
      h("li", {}, "Chaque composant de votre PC (processeur, carte graphique, disques…) a une ligne avec sa température."),
      h("li", {}, h("strong", {}, "Au début"), " est la température habituelle du composant pendant ses 30 premiers jours de suivi. ", h("strong", {}, "Ces 30 derniers jours"), " est sa température habituelle récemment."),
      h("li", {}, `On parle de ${"« stable »"} quand la différence est inférieure à ${String(STABLE_BAND).replace(".", ",")} °C : la plupart des capteurs ne mesurent qu'au degré près.`),
      h("li", {}, "On compare toujours des situations identiques (« Le PC est… ») : la température au repos avec celle au repos, pas avec celle d'un jeu."),
      h("li", {}, "Une saison chaude fait monter légèrement presque tous les composants. Un composant qui chauffe seul, lui, peut signaler de la poussière, une pâte thermique usée ou un ventilateur fatigué."),
    ),
  );
  if (!helpWasClosed()) details.setAttribute("open", "");
  details.addEventListener("toggle", () => rememberHelp(!(details as HTMLDetailsElement).open));
  return details;
}

function mount(): void {
  const app = $("app");
  app.append(
    h(
      "header",
      { class: "top" },
      h("div", {}, h("h1", {}, "Monitoring"), h("p", { class: "subtitle" }, "Les températures de votre PC ont-elles changé depuis le début ?")),
      h("div", { id: "update-slot" }),
    ),
    h("div", { id: "status", "aria-live": "polite" }),
    helpBlock(),
    h("section", { id: "verdict", class: "card", "aria-live": "polite" }),
    h("section", { id: "filters", class: "filters" }),
    h("section", { id: "components", class: "card" }),
    h("section", { id: "detail", class: "card detail", "aria-live": "polite", hidden: true }),
    h("section", { id: "data", class: "card", hidden: true }),
    h("footer", { id: "footer" }),
    h("dialog", { id: "update-dialog", "aria-labelledby": "update-title" }),
  );
  const dialog = $("update-dialog") as HTMLDialogElement;
  dialog.addEventListener("click", (e) => {
    if (e.target === dialog) dialog.close(); // click on the backdrop
  });
}

function start(): void {
  mount();
  readHash();
  void refreshStatus().then(refreshData);
  void refreshUpdate();
  setInterval(() => void refreshStatus(), STATUS_REFRESH_MS);
  setInterval(() => void refreshData(), DATA_REFRESH_MS);
  setInterval(() => void refreshUpdate(), UPDATE_REFRESH_MS);
  addEventListener("hashchange", () => {
    readHash();
    void refreshData();
  });
  // Canvas colors are read at draw time: redraw when the OS theme flips.
  matchMedia("(prefers-color-scheme: dark)").addEventListener("change", () => {
    renderComponents();
    renderDetail();
  });
}

start();
