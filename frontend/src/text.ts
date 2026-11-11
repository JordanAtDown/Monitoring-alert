// Everything the page says, in plain French. No term of statistics on the main view.

import type { Load } from "./api";

/** Below this change, a component is called "stable": most sensors only report whole degrees. */
export const STABLE_BAND = 1.5;
/** Days of follow-up before a change can be spoken of (mirrors the server). */
export const MIN_SPAN_DAYS = 60;

export const LOADS: { id: Load; label: string; hint: string; when: string }[] = [
  { id: "idle", label: "Au repos", hint: "moins de 15 % d'utilisation (bureau, navigation, veille)", when: "quand le PC est au repos" },
  { id: "light", label: "Charge légère", hint: "15 à 40 % d'utilisation (vidéo, tâches courantes)", when: "quand le PC est peu sollicité" },
  { id: "moderate", label: "Charge moyenne", hint: "40 à 75 % d'utilisation", when: "quand le PC est moyennement sollicité" },
  { id: "heavy", label: "Charge élevée", hint: "plus de 75 % d'utilisation (jeux, calculs lourds)", when: "quand le PC est très sollicité" },
];

export type Verdict = "warmer" | "stable" | "cooler" | "unknown";

export function verdictOf(delta: number | null): Verdict {
  if (delta === null) return "unknown";
  if (delta >= STABLE_BAND) return "warmer";
  if (delta <= -STABLE_BAND) return "cooler";
  return "stable";
}

export const VERDICT_LABEL: Record<Verdict, string> = {
  warmer: "Plus chaud",
  stable: "Stable",
  cooler: "Plus froid",
  unknown: "Pas assez de recul",
};

// ── Names ────────────────────────────────────────────────────────────────────

const SENSOR_NAMES: Record<string, string> = {
  "Core (Tctl/Tdie)": "Cœurs du processeur",
  "CPU Package": "Processeur (total)",
  "CCD1 (Tdie)": "Puce de calcul du processeur",
  CPU: "Processeur (sonde de la carte mère)",
  "GPU Core": "Cœur de la carte graphique",
  "GPU Memory Junction": "Mémoire de la carte graphique",
  "GPU Hot Spot": "Point chaud de la carte graphique",
  "Composite Temperature": "Température du disque",
  Chipset: "Chipset de la carte mère",
  "VRM MOS": "Alimentation du processeur",
  "VSoC MOS": "Alimentation du processeur (SoC)",
  System: "Intérieur du boîtier",
};

/** A readable name, plus the technical name when it differs (for those who know it). */
export function sensorLabel(sensor: string): { main: string; tech: string | null } {
  const friendly = SENSOR_NAMES[sensor];
  return friendly ? { main: friendly, tech: sensor } : { main: sensor, tech: null };
}

export type Kind = "cpu" | "gpu" | "board" | "disk" | "other";
export const KIND_ORDER: Kind[] = ["cpu", "gpu", "board", "disk", "other"];
const KIND_TITLE: Record<Kind, string> = {
  cpu: "Processeur",
  gpu: "Carte graphique",
  board: "Carte mère",
  disk: "Disque",
  other: "Autre composant",
};

/** Guesses what a piece of hardware is from the sensors it reports and its name. */
export function hardwareKind(hardware: string, sensors: string[]): Kind {
  if (sensors.some((s) => /^(VRM|VSoC|Chipset|System)/i.test(s))) return "board";
  if (sensors.some((s) => /^GPU /i.test(s)) || /nvidia|geforce|radeon|\brtx\b|\bgtx\b/i.test(hardware)) return "gpu";
  if (sensors.some((s) => /Composite Temperature/i.test(s)) || /ssd|nvme|hdd|\bwd\b|seagate|kingston|crucial/i.test(hardware)) return "disk";
  if (sensors.some((s) => /Tdie|Tctl|CPU Package|^Core #/i.test(s))) return "cpu";
  return "other";
}

export function groupTitle(kind: Kind, hardware: string): string {
  return `${KIND_TITLE[kind]} — ${hardware.trim()}`;
}

const KIND_TITLE_PLURAL: Record<Kind, string> = {
  cpu: "Processeurs",
  gpu: "Cartes graphiques",
  board: "Cartes mères",
  disk: "Disques",
  other: "Autres composants",
};

/** Title of a whole type of component: with its model when there is only one, else the plural. */
export function typeTitle(kind: Kind, hardwares: string[]): string {
  return hardwares.length === 1 ? groupTitle(kind, hardwares[0]) : KIND_TITLE_PLURAL[kind];
}

/** How long ago the history starts, in words ("environ 7 mois"). */
export function spanInWords(days: number): string {
  if (days < 45) return `${days} jours`;
  if (days < 365) return `environ ${Math.round(days / 30)} mois`;
  const years = days / 365;
  return `environ ${years.toFixed(1).replace(".", ",")} an${years >= 2 ? "s" : ""}`;
}
