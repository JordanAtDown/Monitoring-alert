// Types mirror the JSON written by backend/src/application/queries.rs.

export type Load = "idle" | "light" | "moderate" | "heavy";

export interface Reading {
  hardware: string;
  sensor: string;
  value: number;
}

export interface Status {
  version: string;
  now: string;
  collector: {
    started_at: string;
    interval_secs: number;
    last_attempt: string | null;
    last_success: string | null;
    last_error: string | null;
    consecutive_failures: number;
    last_sensor_count: number;
  };
  lhm: { host: string; port: number };
  db: {
    path: string;
    history_since: string | null;
    size_bytes: number;
    total_snapshots: number;
    first_ts: string | null;
    last_ts: string | null;
    raw_retention_days: number;
  };
  /** What is kept, for how long and where, in words. */
  data: { title: string; text: string }[];
  latest: {
    ts: string;
    cpu_load: number | null;
    gpu_load: number | null;
    load_cat: Load;
    readings: Reading[];
  } | null;
}

export type Level = "ok" | "info" | "warn" | "bad";

export interface Check {
  id: string;
  title: string;
  level: Level;
  message: string;
}

export interface Health {
  overall: Level;
  summary: string;
  checks: Check[];
}

export interface Release {
  version: string;
  page_url: string;
  download_url: string | null;
  notes: string | null;
  published_at: string | null;
}

export interface UpdateInfo {
  current: string;
  enabled: boolean;
  available: boolean;
  latest: Release | null;
  checked_at: string | null;
  error: string | null;
  /** One line for the footer. */
  line: string;
  /** Text of the update button, when there is something to update to. */
  button: string | null;
  published: string | null;
}

/** How a sensor or a component changed since the beginning: the word and the figure, already written. */
export interface Verdict {
  id: "warmer" | "stable" | "cooler" | "unknown";
  label: string;
  detail: string | null;
}

export interface SensorView {
  hardware: string;
  sensor: string;
  name: string;
  tech: string | null;
  baseline: string | null;
  current: string | null;
  verdict: Verdict;
  sparkline: number[];
}

export interface ComponentView {
  key: string;
  kind: string;
  title: string;
  subtitle: string | null;
  /** Position among the components: gives the color. */
  slot: number;
  main: SensorView;
  sensors: SensorView[];
}

export interface Overview {
  load: { id: Load; label: string; note: string; choices: { id: Load; label: string }[] };
  headline: {
    state: "empty" | "few" | "ok";
    lead: string;
    detail: string | null;
    tiles: { verdict: Verdict["id"]; count: number; words: string }[];
  };
  components: ComponentView[];
}

export interface Reference {
  value: number;
  label: string;
}

export interface CurveSeries {
  key: string;
  title: string;
  sensor_name: string;
  slot: number;
  now: string | null;
  available: boolean;
  note: string | null;
  points: { day: string; value: number }[];
  reference: Reference | null;
}

export type CurvesMode = "temp" | "delta";

export interface Curves {
  mode: CurvesMode;
  hint: string;
  series: CurveSeries[];
  months: { label: string; cells: (string | null)[] }[];
}

export interface SensorPage {
  name: string;
  subtitle: string;
  slot: number | null;
  story: { kind: "story" | "advice"; spans: { text: string; strong: boolean }[] }[];
  rolling: { day: string; value: number }[];
  reference: Reference | null;
  legend: string[];
  months: { label: string; days: number; usual: string; lowest: string; highest: string }[];
  technical: { label: string; text: string }[];
}

async function getJson<T>(path: string, params: Record<string, string> = {}): Promise<T> {
  const query = new URLSearchParams(params).toString();
  const response = await fetch(query ? `${path}?${query}` : path, { cache: "no-store" });
  if (!response.ok) throw new Error(`${path}: HTTP ${response.status}`);
  return (await response.json()) as T;
}

export const api = {
  status: () => getJson<Status>("/api/status"),
  update: () => getJson<UpdateInfo>("/api/update"),
  health: () => getJson<Health>("/api/health"),
  overview: (load: Load) => getJson<Overview>("/api/overview", { load }),
  curves: (load: Load, mode: CurvesMode) => getJson<Curves>("/api/curves", { load, mode }),
  sensor: (hardware: string, sensor: string, load: Load) => getJson<SensorPage>("/api/sensor", { hardware, sensor, load }),
};
