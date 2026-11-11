/** Colors of the current theme, read from the CSS custom properties (see style.css). */
export interface Theme {
  surface: string;
  ink: string;
  inkSecondary: string;
  inkMuted: string;
  grid: string;
  axis: string;
  /** Categorical slot 1 — the sensor being looked at. */
  series: string;
  /** Diverging poles: warmer than the baseline / cooler than the baseline. */
  warm: string;
  cool: string;
}

export function readTheme(): Theme {
  const style = getComputedStyle(document.documentElement);
  const get = (name: string): string => style.getPropertyValue(name).trim();
  return {
    surface: get("--surface"),
    ink: get("--ink"),
    inkSecondary: get("--ink-secondary"),
    inkMuted: get("--ink-muted"),
    grid: get("--grid"),
    axis: get("--axis"),
    series: get("--series-1"),
    warm: get("--diverge-warm"),
    cool: get("--diverge-cool"),
  };
}

/** `#rrggbb` + alpha → `rgba(...)`. */
export function withAlpha(hex: string, alpha: number): string {
  const h = hex.replace("#", "");
  const v = parseInt(h.length === 3 ? h.replace(/./g, "$&$&") : h, 16);
  return `rgba(${(v >> 16) & 255}, ${(v >> 8) & 255}, ${v & 255}, ${alpha})`;
}
