import "./style.css";
import { api } from "./api";
import { buildComponents } from "./components";
import { clear, h } from "./dom";
import { disposeCharts, hashFor, PAGES, readHash, state, trendKey, writeHash, type Actions, type Selection } from "./state";
import { fillUpdateDialog, renderUpdateButton, versionLine } from "./update";
import { renderCurves } from "./views/curves";
import { renderDetails } from "./views/details";
import { renderHome } from "./views/home";
import { renderPill, renderStatus, setTabTitle } from "./views/status";
import type { Load } from "./api";

const STATUS_REFRESH_MS = 30_000;
const DATA_REFRESH_MS = 10 * 60_000;
const UPDATE_REFRESH_MS = 30 * 60_000;

const $ = (id: string): HTMLElement => document.getElementById(id) as HTMLElement;

// ── Which daily curves the current page needs ────────────────────────────────

function neededTrends(): Selection[] {
  if (state.page === "curves") return state.components.map((c) => ({ hardware: c.main.hardware, sensor: c.main.sensor }));
  if (state.page === "details" && state.selected) return [state.selected];
  return [];
}

const missing = (): Selection[] => neededTrends().filter((s) => !state.trends.has(trendKey(state.load, s.hardware, s.sensor)));
const inflight = new Set<string>();
/** False until the first figures arrive: better "Chargement…" than "no data". */
let loaded = false;

/** Fetches the curves the page lacks, then draws the page again. */
async function ensureTrends(): Promise<void> {
  const load = state.load;
  const todo = missing().filter((s) => !inflight.has(trendKey(load, s.hardware, s.sensor)));
  if (todo.length === 0) return;
  await Promise.all(
    todo.map(async (s) => {
      const key = trendKey(load, s.hardware, s.sensor);
      inflight.add(key);
      try {
        const trend = await api.trend(s.hardware, s.sensor, load);
        if (state.load === load) state.trends.set(key, trend);
      } catch {
        if (state.load === load) state.trends.set(key, null);
      } finally {
        inflight.delete(key);
      }
    }),
  );
  if (state.load === load && missing().length === 0) renderPage();
}

// ── Drawing ──────────────────────────────────────────────────────────────────

const actions: Actions = {
  setLoad(load: Load) {
    state.load = load;
    state.selected = null;
    writeHash();
    void refreshData();
  },
  select(sel) {
    state.selected = sel;
    writeHash();
    renderPage();
    void ensureTrends();
  },
};

function renderChrome(): void {
  setTabTitle();
  renderPill($("pill"), hashFor("status"));
  const tabs = $("tabs");
  clear(tabs);
  for (const page of PAGES) {
    tabs.append(h("a", { class: "tab", href: hashFor(page.id), "aria-current": page.id === state.page ? "page" : undefined }, page.label));
  }
  clear($("footer"));
  $("footer").append(h("p", {}, versionLine(state.status?.version ?? null, state.update)));
  const dialog = $("update-dialog") as HTMLDialogElement;
  renderUpdateButton($("update-slot"), state.update, dialog);
  fillUpdateDialog(dialog, state.update);
}

function renderPage(): void {
  disposeCharts();
  const host = $("page");
  if (state.only && state.components.length > 0) {
    // Coming from a card of the home page: show that component alone.
    state.hidden = new Set(state.components.filter((c) => c.key !== state.only).map((c) => c.key));
    state.only = null;
    writeHash();
  }
  if (!loaded && state.page !== "status") {
    clear(host);
    host.append(h("section", { class: "card" }, h("p", { class: "muted" }, "Chargement…")));
    return;
  }
  switch (state.page) {
    case "home":
      return renderHome(host, actions);
    case "curves":
      if (missing().length > 0) {
        clear(host);
        host.append(h("section", { class: "card" }, h("h2", {}, "Courbes"), h("p", { class: "muted" }, "Chargement des courbes…")));
        return;
      }
      return renderCurves(host, actions);
    case "details":
      return renderDetails(host, actions);
    case "status":
      return renderStatus(host);
  }
}

// ── Data flow ────────────────────────────────────────────────────────────────

async function refreshStatus(): Promise<void> {
  try {
    state.status = await api.status();
  } catch {
    state.status = null;
  }
  renderChrome();
  if (state.page === "home" || state.page === "status") renderPage();
}

async function refreshHealth(): Promise<void> {
  try {
    state.health = await api.health();
  } catch {
    state.health = null;
  }
  renderChrome();
  if (state.page === "status") renderPage();
}

async function refreshUpdate(): Promise<void> {
  try {
    state.update = await api.update();
  } catch {
    state.update = null;
  }
  renderChrome();
}

async function refreshData(): Promise<void> {
  const load = state.load;
  try {
    state.summary = await api.summary(load);
  } catch {
    state.summary = [];
  }
  if (state.load !== load) return; // the reader changed the level meanwhile
  loaded = true;
  state.trends.clear();
  state.components = buildComponents(state.summary, state.status?.latest?.readings ?? []);
  renderChrome();
  renderPage();
  await ensureTrends();
}

function mount(): void {
  $("app").append(
    h(
      "header",
      { class: "top" },
      h("h1", {}, "Monitoring"),
      h("div", { class: "top-right" }, h("div", { id: "pill" }), h("div", { id: "update-slot" })),
    ),
    h("nav", { id: "tabs", class: "tabs", "aria-label": "Pages" }),
    h("main", { id: "page" }),
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
  renderChrome();
  void refreshHealth();
  void refreshStatus().then(refreshData);
  void refreshUpdate();
  setInterval(() => void refreshHealth(), STATUS_REFRESH_MS);
  setInterval(() => void refreshStatus(), STATUS_REFRESH_MS);
  setInterval(() => void refreshData(), DATA_REFRESH_MS);
  setInterval(() => void refreshUpdate(), UPDATE_REFRESH_MS);
  addEventListener("hashchange", () => {
    const before = state.load;
    readHash();
    if (state.load !== before) {
      void refreshData();
      return;
    }
    renderChrome();
    renderPage();
    void ensureTrends();
  });
  // Canvas colors are read at draw time: redraw when the OS theme flips.
  matchMedia("(prefers-color-scheme: dark)").addEventListener("change", () => renderPage());
}

start();
