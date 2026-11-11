import "./style.css";
import { api, type CurvesMode, type Load } from "./api";
import { clear, h } from "./dom";
import { disposeCharts, hashFor, PAGES, readHash, sensorKey, state, writeHash, type Actions, type Selection } from "./state";
import { fillUpdateDialog, renderUpdateButton } from "./update";
import { renderCurves } from "./views/curves";
import { renderDetails } from "./views/details";
import { renderHome } from "./views/home";
import { renderPill, renderStatus, setTabTitle } from "./views/status";

// The page decides nothing: every figure, word and sentence comes from the backend (/api/…), which
// groups the sensors, writes the sentences and formats the numbers. This file fetches and lays out.

const STATUS_REFRESH_MS = 30_000;
const DATA_REFRESH_MS = 10 * 60_000;
const UPDATE_REFRESH_MS = 30 * 60_000;

const $ = (id: string): HTMLElement => document.getElementById(id) as HTMLElement;

/** False until the first figures arrive: better "Chargement…" than "no data". */
let loaded = false;

// ── What the current page still has to fetch ─────────────────────────────────

function pending(): boolean {
  if (state.page === "curves") return !state.curves.has(state.mode);
  if (state.page === "details" && state.selected) return !state.sensors.has(sensorKey(state.selected));
  return false;
}

const inflight = new Set<string>();

/** Fetches what the page lacks, then draws it again. */
async function ensureFetched(): Promise<void> {
  const { load, mode, selected, page } = state;
  if (page === "curves" && !state.curves.has(mode) && !inflight.has(`c|${load}|${mode}`)) {
    const key = `c|${load}|${mode}`;
    inflight.add(key);
    try {
      const data = await api.curves(load, mode);
      if (state.load === load) state.curves.set(mode, data);
    } catch {
      if (state.load === load) state.curves.set(mode, null);
    } finally {
      inflight.delete(key);
    }
  }
  if (page === "details" && selected && !state.sensors.has(sensorKey(selected)) && !inflight.has(`s|${load}|${sensorKey(selected)}`)) {
    const key = `s|${load}|${sensorKey(selected)}`;
    inflight.add(key);
    try {
      const data = await api.sensor(selected.hardware, selected.sensor, load);
      if (state.load === load) state.sensors.set(sensorKey(selected), data);
    } catch {
      if (state.load === load) state.sensors.set(sensorKey(selected), null);
    } finally {
      inflight.delete(key);
    }
  }
  if (state.load === load && state.page === page && !pending()) renderPage();
}

// ── Drawing ──────────────────────────────────────────────────────────────────

const actions: Actions = {
  setLoad(load: Load) {
    state.load = load;
    state.selected = null;
    writeHash();
    void refreshData();
  },
  select(sel: Selection | null) {
    state.selected = sel;
    writeHash();
    renderPage();
    void ensureFetched();
  },
  setMode(mode: CurvesMode) {
    state.mode = mode;
    renderPage();
    void ensureFetched();
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
  const footer = $("footer");
  clear(footer);
  footer.append(h("p", {}, state.update?.line ?? `Monitoring ${state.status?.version ?? ""}`.trim()));
  const dialog = $("update-dialog") as HTMLDialogElement;
  renderUpdateButton($("update-slot"), state.update, dialog);
  fillUpdateDialog(dialog, state.update);
}

function loadingCard(title?: string): HTMLElement {
  return h("section", { class: "card" }, title ? h("h2", {}, title) : null, h("p", { class: "muted" }, "Chargement…"));
}

function renderPage(): void {
  disposeCharts();
  const host = $("page");
  if (!loaded && state.page !== "status") {
    clear(host);
    host.append(loadingCard());
    return;
  }
  const overview = state.overview;
  if (state.only && overview) {
    // Coming from a card of the home page: show that component alone.
    state.hidden = new Set(overview.components.filter((c) => c.key !== state.only).map((c) => c.key));
    state.only = null;
    writeHash();
  }
  switch (state.page) {
    case "home":
      return renderHome(host, actions);
    case "curves": {
      const data = state.curves.get(state.mode);
      if (data === undefined) {
        clear(host);
        host.append(loadingCard("Courbes"));
        return;
      }
      if (data === null) {
        clear(host);
        host.append(h("section", { class: "card" }, h("h2", {}, "Courbes"), h("p", { class: "muted" }, "Impossible de charger les courbes.")));
        return;
      }
      return renderCurves(host, data, actions);
    }
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
  if (state.page === "status") renderPage();
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
  // Fetched together; one failing never takes the other down with it (nor the page).
  const [overview, cooling] = await Promise.all([api.overview(load).catch(() => null), api.cooling(load).catch(() => null)]);
  if (state.load !== load) return; // the reader changed the level meanwhile
  loaded = true;
  state.overview = overview;
  state.cooling = cooling;
  state.curves.clear();
  state.sensors.clear();
  renderChrome();
  renderPage();
  await ensureFetched();
}

function mount(): void {
  $("app").append(
    h("header", { class: "top" }, h("h1", {}, "Monitoring"), h("div", { class: "top-right" }, h("div", { id: "pill" }), h("div", { id: "update-slot" }))),
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
    void ensureFetched();
  });
  // Canvas colors are read at draw time: redraw when the OS theme flips.
  matchMedia("(prefers-color-scheme: dark)").addEventListener("change", () => renderPage());
}

start();
