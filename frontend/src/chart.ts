import uPlot from "uplot";
import "uplot/dist/uPlot.min.css";
import { clear, h } from "./dom";
import { fmtLongDay, fmtMonthShort, fmtSigned, fmtTemp, parseDay } from "./format";
import { readTheme, withAlpha, type Theme } from "./theme";

const DAY = 86400;
const MONTH_LABEL_MIN_PX = 38;

/** First instant of each month intersecting [from, to], as unix seconds (UTC). */
function monthStarts(from: number, to: number): number[] {
  const first = new Date(from * 1000);
  let y = first.getUTCFullYear();
  let m = first.getUTCMonth();
  const out: number[] = [];
  for (;;) {
    const t = Date.UTC(y, m, 1) / 1000;
    if (t > to) return out;
    out.push(t);
    m += 1;
    if (m > 11) {
      m = 0;
      y += 1;
    }
  }
}

/** One line of the chart: a value per day (the 30-day usual temperature of one sensor). */
export interface ChartSeries {
  id: string;
  label: string;
  color: string;
  /** "YYYY-MM-DD" → value */
  values: Map<string, number>;
}

export interface ChartInput {
  series: ChartSeries[];
  /** `temp`: values are °C. `delta`: values are the change since the beginning (0 = the beginning). */
  unit: "temp" | "delta";
  /** A dashed horizontal line, and what to call it. */
  reference: { value: number; label: string } | null;
}

const FONT = 'system-ui, -apple-system, "Segoe UI", sans-serif';

/** Axis number with the decimals the step needs, a real minus sign, and "+" in `delta` mode. */
function axisNumber(v: number, decimals: number, signed: boolean): string {
  const text = Math.abs(v).toFixed(decimals).replace(".", ",");
  return `${v < 0 ? "−" : signed && v > 0 ? "+" : ""}${text} °`;
}

/**
 * The curves: one line per series, nothing else. Months alternate light/dark in the background so a
 * seasonal swing is easy to recognise. The legend and the choice of lines live in the page (HTML).
 */
export function drawChart(host: HTMLElement, input: ChartInput): () => void {
  clear(host);
  const theme = readTheme();
  const delta = input.unit === "delta";
  const fmtValue = delta ? (v: number) => fmtSigned(v) : fmtTemp;

  const days = [...new Set(input.series.flatMap((s) => [...s.values.keys()]))].sort();
  const xs = days.map(parseDay);
  const columns = input.series.map((s) => days.map((d) => s.values.get(d) ?? null));

  const all = columns.flat().filter((v): v is number => v !== null);
  let lo = Math.min(...all);
  let hi = Math.max(...all);
  if (input.reference) {
    lo = Math.min(lo, input.reference.value);
    hi = Math.max(hi, input.reference.value);
  }
  const range: [number, number] = delta ? [Math.min(Math.floor(lo - 0.5), -1), Math.max(Math.ceil(hi + 0.5), 1)] : [Math.floor(lo - 1), Math.ceil(hi + 1)];

  const tooltip = h("div", { class: "tooltip", role: "presentation", hidden: true });
  const frame = h("div", { class: "chart-frame" }, tooltip);
  host.append(frame);

  const monthBands = (u: uPlot, t: Theme): void => {
    const { left, top, width, height } = u.bbox;
    const [min, max] = [u.scales.x.min ?? xs[0], u.scales.x.max ?? xs[xs.length - 1]];
    const ctx = u.ctx;
    const dpr = devicePixelRatio;
    ctx.save();
    ctx.beginPath();
    ctx.rect(left, top, width, height);
    ctx.clip();
    for (const start of monthStarts(min, max)) {
      const d = new Date(start * 1000);
      const next = Date.UTC(d.getUTCFullYear(), d.getUTCMonth() + 1, 1) / 1000;
      const x0 = u.valToPos(start, "x", true);
      const x1 = u.valToPos(next, "x", true);
      const month = d.getUTCMonth();
      if (month % 2 === 1) {
        ctx.fillStyle = withAlpha(t.ink, 0.045);
        ctx.fillRect(x0, top, x1 - x0, height);
      }
      if ((x1 - x0) / dpr >= MONTH_LABEL_MIN_PX) {
        ctx.fillStyle = t.inkMuted;
        ctx.font = `${11 * dpr}px ${FONT}`;
        ctx.textBaseline = "top";
        ctx.fillText(month === 0 ? `${fmtMonthShort(start)} ${d.getUTCFullYear()}` : fmtMonthShort(start), Math.max(x0, left) + 4 * dpr, top + 4 * dpr);
      }
    }
    ctx.restore();
  };

  const referenceLabel = (u: uPlot, t: Theme): void => {
    if (!input.reference) return;
    const { left } = u.bbox;
    const dpr = devicePixelRatio;
    const ctx = u.ctx;
    ctx.save();
    ctx.font = `${12 * dpr}px ${FONT}`;
    ctx.textBaseline = "bottom";
    ctx.fillStyle = t.inkSecondary;
    ctx.fillText(input.reference.label, left + 6 * dpr, u.valToPos(input.reference.value, "y", true) - 5 * dpr);
    ctx.restore();
  };

  const series: uPlot.Series[] = [
    {},
    ...input.series.map((s): uPlot.Series => ({ label: s.label, stroke: s.color, width: 2.5, spanGaps: true, points: { show: false } })),
  ];
  const data: (number | null)[][] = [xs, ...columns];
  if (input.reference) {
    series.push({ label: input.reference.label, stroke: theme.inkSecondary, width: 1.5, dash: [6, 4], points: { show: false } });
    data.push(days.map(() => (input.reference as { value: number }).value));
  }

  const axisBase = {
    stroke: theme.inkMuted,
    grid: { stroke: theme.grid, width: 1 },
    ticks: { stroke: theme.axis, width: 1, size: 4 },
    font: `12px ${FONT}`,
  };

  const size = (): { width: number; height: number } => ({ width: Math.max(frame.clientWidth, 320), height: 360 });
  const opts: uPlot.Options = {
    ...size(),
    padding: [14, 16, 0, 0],
    legend: { show: false },
    tzDate: (ts) => uPlot.tzDate(new Date(ts * 1000), "Etc/UTC"),
    scales: { x: { time: true }, y: { range: () => range } },
    cursor: { x: true, y: false, drag: { x: true, y: false, setScale: true } },
    select: { show: true, left: 0, top: 0, width: 0, height: 0 },
    axes: [
      {
        ...axisBase,
        values: (_u, splits) =>
          splits.map((s, i) => {
            const step = i > 0 ? s - splits[i - 1] : (splits[1] ?? s) - s;
            const d = new Date(s * 1000);
            return step < 28 * DAY ? `${d.getUTCDate()} ${fmtMonthShort(s)}` : `${fmtMonthShort(s)} ${String(d.getUTCFullYear()).slice(2)}`;
          }),
      },
      {
        ...axisBase,
        size: 64,
        values: (_u, splits) => {
          const step = splits.length > 1 ? Math.abs(splits[1] - splits[0]) : 1;
          return splits.map((v) => axisNumber(v, step < 1 ? 1 : 0, delta));
        },
      },
    ],
    series,
    hooks: {
      drawClear: [(u) => monthBands(u, theme)],
      draw: [(u) => referenceLabel(u, theme)],
      setCursor: [
        (u) => {
          const idx = u.cursor.idx;
          if (idx === null || idx === undefined || u.cursor.left === undefined || u.cursor.left < 0) {
            tooltip.hidden = true;
            return;
          }
          clear(tooltip);
          const rows = input.series
            .map((s, k) => ({ s, value: columns[k][idx] }))
            .filter((r): r is { s: ChartSeries; value: number } => r.value !== null)
            .sort((a, b) => b.value - a.value);
          tooltip.append(h("div", { class: "tt-date" }, fmtLongDay(xs[idx])));
          for (const { s, value } of rows) tooltip.append(ttRow(s.label, fmtValue(value), s.color));
          if (rows.length === 0) tooltip.append(h("div", { class: "tt-n" }, "Pas de mesure ce jour-là"));
          tooltip.hidden = false;
          const left = (u.cursor.left ?? 0) + u.bbox.left / devicePixelRatio;
          const flip = left > frame.clientWidth * 0.6;
          tooltip.style.left = `${flip ? left - tooltip.offsetWidth - 14 : left + 14}px`;
          tooltip.style.top = "36px";
        },
      ],
    },
  };

  const plot = new uPlot(opts, data as uPlot.AlignedData, frame);
  const resize = new ResizeObserver(() => plot.setSize(size()));
  resize.observe(frame);
  return () => {
    resize.disconnect();
    plot.destroy();
  };
}

/** A tooltip line: a short stroke of the series color, the value first, then the name. */
function ttRow(label: string, value: string, color: string): HTMLElement {
  const swatch = h("span", { class: "tt-key" });
  swatch.style.background = color;
  return h("div", { class: "tt-row" }, swatch, h("span", { class: "tt-value tt-strong" }, value), h("span", { class: "tt-label" }, label));
}
