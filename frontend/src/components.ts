// What the machine is made of, as a person names it: Processeur, Carte graphique, Carte mère, Boîtier,
// Disque 1, Disque 2… LibreHardwareMonitor reports sensors per piece of *hardware* (the motherboard
// chip reports the processor probe and the case probe too), so each sensor is placed on its own.

import type { Reading, Summary } from "./api";

export type Kind = "cpu" | "gpu" | "board" | "case" | "disk" | "other";
export const KIND_ORDER: Kind[] = ["cpu", "gpu", "board", "case", "disk", "other"];

const KIND_TITLE: Record<Kind, string> = {
  cpu: "Processeur",
  gpu: "Carte graphique",
  board: "Carte mère",
  case: "Boîtier",
  disk: "Disque",
  other: "Autre composant",
};

/** Kinds of which a PC can have several: each piece of hardware is its own component, numbered. */
const NUMBERED: Kind[] = ["gpu", "disk"];

export interface Component {
  /** Stable identifier (used in the URL and to keep a color per component). */
  key: string;
  kind: Kind;
  /** "Processeur", "Disque 2"… */
  title: string;
  /** The model, when it says something ("AMD Ryzen 7 5700X"). */
  subtitle: string | null;
  /** All its sensors, the one that stands for the component first. */
  rows: Summary[];
  /** The sensor that stands for the component on the home page and on the curves. */
  main: Summary;
}

// ── Classification ───────────────────────────────────────────────────────────

/** What a whole piece of hardware is, guessed from its sensors and its name. */
export function hardwareKind(hardware: string, sensors: string[]): Kind {
  // The sensors say the most; the name only decides when they are not enough.
  if (sensors.some((s) => /^(VRM|VSoC|Chipset|System|Motherboard|PCH)/i.test(s))) return "board";
  if (sensors.some((s) => /^GPU /i.test(s))) return "gpu";
  if (sensors.some((s) => /Composite Temperature/i.test(s))) return "disk";
  if (sensors.some((s) => /Tdie|Tctl|CPU Package|^Core #/i.test(s))) return "cpu";
  if (/nvidia|geforce|radeon|\brtx\b|\bgtx\b/i.test(hardware)) return "gpu";
  if (/ssd|nvme|hdd|\bwd\b|seagate|kingston|crucial|samsung|\bct\d/i.test(hardware)) return "disk";
  if (/gigabyte|aorus|asus|\bmsi\b|asrock|biostar|supermicro/i.test(hardware)) return "board";
  return "other";
}

/** The motherboard chip also reports the processor probe and the probe inside the case. */
function sensorKind(hardwareKindOf: Kind, sensor: string): Kind {
  if (hardwareKindOf !== "board") return hardwareKindOf;
  if (/^CPU/i.test(sensor)) return "cpu";
  if (/^(System|Case|Chassis|Ambient)/i.test(sensor)) return "case";
  return "board";
}

/** Order in which a component's own sensor is preferred to stand for it. */
const MAIN_SENSOR: Record<Kind, RegExp[]> = {
  cpu: [/^Core \(Tctl|^CPU Package/i, /Tctl|Tdie/i],
  gpu: [/^GPU Core/i],
  board: [/^Chipset/i],
  case: [/^System/i],
  disk: [/^Composite Temperature/i, /^Temperature$/i],
  other: [],
};

function pickMain(kind: Kind, rows: Summary[]): Summary {
  for (const pattern of MAIN_SENSOR[kind]) {
    const hit = rows.find((r) => pattern.test(r.sensor));
    if (hit) return hit;
  }
  return rows[0];
}

// ── Building the list ────────────────────────────────────────────────────────

const byChange = (a: Summary, b: Summary): number => (b.delta ?? -Infinity) - (a.delta ?? -Infinity);
const bySensor = (a: Summary, b: Summary): number => a.sensor.localeCompare(b.sensor);

/**
 * Groups the sensors into the components of the PC. `seen` lists every sensor the machine reports
 * (the numbering of the disks must not change when one of them has no figures at this load level).
 */
export function buildComponents(summary: Summary[], seen: Pick<Reading, "hardware" | "sensor">[] = []): Component[] {
  const sensorsOf = new Map<string, Set<string>>();
  for (const s of [...seen, ...summary]) sensorsOf.set(s.hardware, (sensorsOf.get(s.hardware) ?? new Set()).add(s.sensor));
  const hardwareKinds = new Map([...sensorsOf].map(([hw, sensors]) => [hw, hardwareKind(hw, [...sensors])]));

  // Numbering of the hardware of a numbered kind, in alphabetical order of the model.
  const numbering = new Map<string, number>();
  for (const kind of NUMBERED) {
    const models = [...hardwareKinds].filter(([, k]) => k === kind).map(([hw]) => hw).sort((a, b) => a.trim().localeCompare(b.trim()));
    models.forEach((hw, i) => numbering.set(hw, i + 1));
  }
  const countOf = (kind: Kind): number => [...hardwareKinds.values()].filter((k) => k === kind).length;

  const groups = new Map<string, { kind: Kind; title: string; models: Set<string>; rows: Summary[]; order: number }>();
  for (const row of summary) {
    const owner = hardwareKinds.get(row.hardware) ?? "other";
    const kind = sensorKind(owner, row.sensor);
    const numbered = NUMBERED.includes(kind);
    const key = numbered || kind === "other" ? `${kind}:${row.hardware.trim()}` : kind;
    let title = KIND_TITLE[kind];
    if (numbered && countOf(kind) > 1) title += ` ${numbering.get(row.hardware) ?? ""}`.trimEnd();
    if (kind === "other") title = row.hardware.trim();
    const group = groups.get(key) ?? { kind, title, models: new Set<string>(), rows: [], order: numbering.get(row.hardware) ?? 0 };
    // The model is shown only when the sensor really belongs to that hardware (not the board's CPU probe).
    if (kind === owner && kind !== "other") group.models.add(row.hardware.trim());
    group.rows.push(row);
    groups.set(key, group);
  }

  return [...groups]
    .sort(([, a], [, b]) => KIND_ORDER.indexOf(a.kind) - KIND_ORDER.indexOf(b.kind) || a.order - b.order || a.title.localeCompare(b.title))
    .map(([key, g]) => {
      const main = pickMain(g.kind, g.rows);
      // The sensor that stands for the component comes first, then the others, the most changed first.
      const others = g.rows.filter((r) => r !== main).sort(g.kind === "disk" || g.kind === "gpu" ? bySensor : byChange);
      return { key, kind: g.kind, title: g.title, subtitle: g.models.size > 0 ? [...g.models].join(", ") : null, rows: [main, ...others], main };
    });
}

// ── Sensor names ─────────────────────────────────────────────────────────────

const SENSOR_NAMES: Record<string, string> = {
  "Core (Tctl/Tdie)": "Cœurs du processeur",
  "CPU Package": "Processeur (total)",
  "CCD1 (Tdie)": "Puce de calcul du processeur",
  CPU: "Sonde de la carte mère, près du processeur",
  "GPU Core": "Cœur de la carte graphique",
  "GPU Memory Junction": "Mémoire de la carte graphique",
  "GPU Hot Spot": "Point chaud de la carte graphique",
  Chipset: "Chipset de la carte mère",
  "VRM MOS": "Alimentation du processeur",
  "VSoC MOS": "Alimentation du processeur (SoC)",
  System: "Intérieur du boîtier",
};

/** A readable name, plus the technical name when it differs (for those who know it). */
export function sensorLabel(sensor: string, kind?: Kind): { main: string; tech: string | null } {
  if (kind === "disk") {
    if (/^(Composite Temperature|Temperature)$/.test(sensor)) return { main: "Température du disque", tech: sensor };
    const n = /^Temperature #(\d+)$/.exec(sensor);
    if (n) return { main: `Sonde interne n° ${n[1]}`, tech: sensor };
  }
  const friendly = SENSOR_NAMES[sensor];
  return friendly ? { main: friendly, tech: sensor } : { main: sensor, tech: null };
}
