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
  advice: string | null;
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
}

export interface Point {
  day: string;
  median: number;
  min: number;
  p95: number;
  max: number;
  n: number;
}

export interface Trend {
  points: Point[];
  rolling: { day: string; value: number }[];
  days: number;
  span_days: number;
  sufficient: boolean;
  baseline: number | null;
  baseline_from: string | null;
  baseline_to: string | null;
  delta: number | null;
  slope_per_month: number | null;
  fit_start: number | null;
}

export interface Summary {
  hardware: string;
  sensor: string;
  load_cat: Load;
  days: number;
  span_days: number;
  sufficient: boolean;
  baseline: number | null;
  current: number | null;
  delta: number | null;
  slope_per_month: number | null;
  sparkline: number[];
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
  summary: (load: Load) => getJson<Summary[]>("/api/summary", { load }),
  trend: (hardware: string, sensor: string, load: Load) =>
    getJson<Trend>("/api/trend", { hardware, sensor, load }),
};
