// What the page knows at a given moment, shared by the views. The views only read it; main.ts changes it.

import type { Curves, CurvesMode, Health, Level, Load, Overview, SensorPage, Status, UpdateInfo } from "./api";
import { readTheme, SERIES_COUNT } from "./theme";

export type Page = "home" | "curves" | "details" | "status";
export const PAGES: { id: Page; label: string }[] = [
  { id: "home", label: "Accueil" },
  { id: "curves", label: "Courbes" },
  { id: "details", label: "Détails" },
  { id: "status", label: "État" },
];

const LOAD_IDS: Load[] = ["idle", "light", "moderate", "heavy"];

export interface Selection {
  hardware: string;
  sensor: string;
}

export const state = {
  page: "home" as Page,
  load: "idle" as Load,
  /** The sensor opened on the Details page. */
  selected: null as Selection | null,
  /** Set by a link from the home page: show only this component on the curves. */
  only: null as string | null,
  status: null as Status | null,
  health: null as Health | null,
  update: null as UpdateInfo | null,
  /** The home page at the current load level (the choices of load levels come with it). */
  overview: null as Overview | null,
  /** Components the reader switched off on the curves. */
  hidden: new Set<string>(),
  mode: "temp" as CurvesMode,
  /** Pages already fetched for the current load level; `null` = the fetch failed. */
  curves: new Map<CurvesMode, Curves | null>(),
  sensors: new Map<string, SensorPage | null>(),
};

/** What the views can ask the page to do. */
export interface Actions {
  setLoad(load: Load): void;
  /** Opens (or closes, with null) a sensor on the Details page. */
  select(sel: Selection | null): void;
  setMode(mode: CurvesMode): void;
}

export const sensorKey = (s: Selection): string => `${s.hardware}|${s.sensor}`;

// ── Charts alive on the page ─────────────────────────────────────────────────

const disposers: (() => void)[] = [];
export function trackChart(dispose: () => void): void {
  disposers.push(dispose);
}
export function disposeCharts(): void {
  for (const dispose of disposers.splice(0)) dispose();
}

// ── Address ──────────────────────────────────────────────────────────────────

export function hashFor(page: Page, extra: Record<string, string> = {}): string {
  return `#${new URLSearchParams({ page, load: state.load, ...extra }).toString()}`;
}

export function readHash(): void {
  const params = new URLSearchParams(location.hash.slice(1));
  const page = params.get("page");
  state.page = PAGES.some((p) => p.id === page) ? (page as Page) : "home";
  const load = params.get("load");
  if (LOAD_IDS.includes(load as Load)) state.load = load as Load;
  const hardware = params.get("hw");
  const sensor = params.get("sensor");
  state.selected = hardware && sensor ? { hardware, sensor } : null;
  state.only = params.get("only");
}

export function writeHash(): void {
  const extra: Record<string, string> = {};
  if (state.selected) {
    extra.hw = state.selected.hardware;
    extra.sensor = state.selected.sensor;
  }
  history.replaceState(null, "", hashFor(state.page, extra));
}

// ── Colors ───────────────────────────────────────────────────────────────────

/** A component keeps its color whatever is shown: it depends on its place in the list. Past 8, gray. */
export function colorVar(index: number): string {
  return index < SERIES_COUNT ? `var(--series-${index + 1})` : "var(--ink-muted)";
}
export function colorValue(index: number): string {
  const theme = readTheme();
  return index < SERIES_COUNT ? theme.palette[index] : theme.inkMuted;
}

export const LEVEL_ICON: Record<Level, string> = { ok: "✓", info: "ℹ", warn: "⚠", bad: "✕" };
