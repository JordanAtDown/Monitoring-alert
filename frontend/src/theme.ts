/** Colors of the current theme, read from the CSS custom properties (see style.css). */
export interface Theme {
  surface: string;
  ink: string;
  inkSecondary: string;
  inkMuted: string;
  grid: string;
  axis: string;
  /** Categorical slots 1–8, in the fixed order of the palette (see style.css). */
  palette: string[];
}

/** Colors of the components: fixed by position in the list, never by what is currently shown. */
export const SERIES_COUNT = 8;

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
    palette: Array.from({ length: SERIES_COUNT }, (_, i) => get(`--series-${i + 1}`)),
  };
}

/** `#rrggbb` + alpha → `rgba(...)`. */
export function withAlpha(hex: string, alpha: number): string {
  const h = hex.replace("#", "");
  const v = parseInt(h.length === 3 ? h.replace(/./g, "$&$&") : h, 16);
  return `rgba(${(v >> 16) & 255}, ${(v >> 8) & 255}, ${v & 255}, ${alpha})`;
}
