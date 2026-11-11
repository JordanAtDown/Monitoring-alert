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

/** How long ago the history starts, in words ("environ 7 mois"). */
export function spanInWords(days: number): string {
  if (days < 45) return `${days} jours`;
  if (days < 365) return `environ ${Math.round(days / 30)} mois`;
  const years = days / 365;
  return `environ ${years.toFixed(1).replace(".", ",")} an${years >= 2 ? "s" : ""}`;
}
