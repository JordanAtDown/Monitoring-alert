import uPlot from "uplot";
import "uplot/dist/uPlot.min.css";
import type { Trend } from "./api";
import { clear, h } from "./dom";
import { fmtInt, fmtLongDay, fmtMonthShort, fmtSigned, fmtTemp, parseDay } from "./format";
import { readTheme, withAlpha, type Theme } from "./theme";

const DAY = 86400;
/** A drift of this many °C paints the shading at full intensity. */
const FULL_SHADE_DELTA = 6;
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

/**
 * The drift chart of one sensor at one load level.
 *
 *  - dots: the median of each day, with a faint min–p95 band behind
 *  - line: the 30-day rolling median — the signal to follow
 *  - dashed: the baseline (median of the first 30 days)
 *  - dotted: the least-squares trend
 *  - shading between the baseline and the rolling median, warm above / cool below
 *  - month bands and labels, so a seasonal swing is easy to recognise
 */
export interface ChartOptions {
  /** The faint band from the lowest to the 95th percentile of each day. */
  band: boolean;
  /** The least-squares line through all the days. */
  trendLine: boolean;
}

export function drawTrendChart(host: HTMLElement, trend: Trend, options: ChartOptions): () => void {
  clear(host);
  const theme = readTheme();

  const xs = trend.points.map((p) => parseDay(p.day));
  const rollingByDay = new Map(trend.rolling.map((r) => [r.day, r.value]));
  const median = trend.points.map((p) => p.median);
  const rolling = trend.points.map((p) => rollingByDay.get(p.day) ?? null);
  const baseline = trend.points.map(() => trend.baseline);
  const slopePerDay = trend.slope_per_month === null ? null : trend.slope_per_month / 30.4375;
  const trendLine = xs.map((x) =>
    slopePerDay === null || trend.fit_start === null
      ? null
      : trend.fit_start + slopePerDay * ((x - xs[0]) / DAY),
  );

  let lo = Math.min(...trend.points.map((p) => p.min));
  let hi = Math.max(...trend.points.map((p) => p.p95));
  if (trend.baseline !== null) {
    lo = Math.min(lo, trend.baseline);
    hi = Math.max(hi, trend.baseline);
  }

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
    const starts = monthStarts(min, max);
    starts.forEach((start, i) => {
      const next = Date.UTC(new Date(start * 1000).getUTCFullYear(), new Date(start * 1000).getUTCMonth() + 1, 1) / 1000;
      const x0 = u.valToPos(start, "x", true);
      const x1 = u.valToPos(next, "x", true);
      const month = new Date(start * 1000).getUTCMonth();
      if (month % 2 === 1) {
        ctx.fillStyle = withAlpha(t.ink, 0.045);
        ctx.fillRect(x0, top, x1 - x0, height);
      }
      if ((x1 - x0) / dpr >= MONTH_LABEL_MIN_PX) {
        const january = month === 0;
        const label = january ? `${fmtMonthShort(start)} ${new Date(start * 1000).getUTCFullYear()}` : fmtMonthShort(start);
        ctx.fillStyle = t.inkMuted;
        ctx.font = `${11 * dpr}px system-ui, -apple-system, "Segoe UI", sans-serif`;
        ctx.textBaseline = "top";
        ctx.fillText(label, Math.max(x0, left) + 4 * dpr, top + 4 * dpr);
      }
      void i;
    });
    ctx.restore();
  };

  const bandAndShading = (u: uPlot, t: Theme): void => {
    const { left, top, width, height } = u.bbox;
    const ctx = u.ctx;
    ctx.save();
    ctx.beginPath();
    ctx.rect(left, top, width, height);
    ctx.clip();

    // min–p95 band, one polygon per run of consecutive days (only when asked for).
    ctx.fillStyle = withAlpha(t.series, 0.1);
    let run: number[] = [];
    const flush = (): void => {
      if (run.length > 1) {
        ctx.beginPath();
        run.forEach((i, k) => {
          const x = u.valToPos(xs[i], "x", true);
          const y = u.valToPos(trend.points[i].p95, "y", true);
          if (k === 0) ctx.moveTo(x, y);
          else ctx.lineTo(x, y);
        });
        for (let k = run.length - 1; k >= 0; k--) {
          ctx.lineTo(u.valToPos(xs[run[k]], "x", true), u.valToPos(trend.points[run[k]].min, "y", true));
        }
        ctx.closePath();
        ctx.fill();
      }
      run = [];
    };
    for (let i = 0; options.band && i < xs.length; i++) {
      if (run.length && xs[i] - xs[run[run.length - 1]] > 2 * DAY) flush();
      run.push(i);
    }
    flush();

    // Drift shading between the baseline and the rolling median.
    if (trend.baseline !== null) {
      const yBase = u.valToPos(trend.baseline, "y", true);
      for (let i = 0; i + 1 < xs.length; i++) {
        const a = rolling[i];
        const b = rolling[i + 1];
        if (a === null || b === null || xs[i + 1] - xs[i] > 7 * DAY) continue;
        const delta = (a + b) / 2 - trend.baseline;
        const strength = Math.min(Math.abs(delta) / FULL_SHADE_DELTA, 1);
        ctx.fillStyle = withAlpha(delta >= 0 ? t.warm : t.cool, 0.1 + 0.32 * strength);
        const x0 = u.valToPos(xs[i], "x", true);
        const x1 = u.valToPos(xs[i + 1], "x", true) + 0.5;
        ctx.beginPath();
        ctx.moveTo(x0, u.valToPos(a, "y", true));
        ctx.lineTo(x1, u.valToPos(b, "y", true));
        ctx.lineTo(x1, yBase);
        ctx.lineTo(x0, yBase);
        ctx.closePath();
        ctx.fill();
      }
    }
    ctx.restore();
  };

  /** Selective direct labels + the end dot (with its surface ring). */
  const endLabels = (u: uPlot, t: Theme): void => {
    const { left, top, width } = u.bbox;
    const dpr = devicePixelRatio;
    const ctx = u.ctx;
    ctx.save();
    ctx.font = `${12 * dpr}px system-ui, -apple-system, "Segoe UI", sans-serif`;
    ctx.textBaseline = "middle";

    let last = -1;
    for (let i = rolling.length - 1; i >= 0; i--) {
      if (rolling[i] !== null) {
        last = i;
        break;
      }
    }
    if (last >= 0) {
      const value = rolling[last] as number;
      const x = u.valToPos(xs[last], "x", true);
      const y = u.valToPos(value, "y", true);
      if (x <= left + width + 1) {
        ctx.beginPath();
        ctx.arc(x, y, 6 * dpr, 0, Math.PI * 2);
        ctx.fillStyle = t.surface;
        ctx.fill();
        ctx.beginPath();
        ctx.arc(x, y, 4 * dpr, 0, Math.PI * 2);
        ctx.fillStyle = t.series;
        ctx.fill();
        ctx.fillStyle = t.ink;
        ctx.fillText(fmtTemp(value), x + 10 * dpr, y);
      }
    }
    if (trend.baseline !== null) {
      const y = u.valToPos(trend.baseline, "y", true);
      ctx.fillStyle = t.inkSecondary;
      ctx.textBaseline = "bottom";
      ctx.fillText(`Au début : ${fmtTemp(trend.baseline)}`, left + 6 * dpr, y - 5 * dpr);
    }
    ctx.restore();
    void top;
  };

  const dash = (width: number, color: string, pattern: number[]): uPlot.Series => ({
    stroke: color,
    width,
    dash: pattern,
    points: { show: false },
  });

  const series: uPlot.Series[] = [
    {},
    {
      label: "Un jour",
      stroke: withAlpha(theme.series, 0.5),
      width: 0,
      paths: () => null,
      points: { show: true, size: 4, width: 0, fill: withAlpha(theme.series, 0.5), stroke: "transparent" },
    },
    { label: "30 derniers jours", stroke: theme.series, width: 2.5, points: { show: false } },
    { label: "Au début", ...dash(1.5, theme.inkSecondary, [6, 4]) },
  ];
  if (options.trendLine) series.push({ label: "Tendance", ...dash(1.5, theme.inkMuted, [2, 3]) });

  const axisBase = {
    stroke: theme.inkMuted,
    grid: { stroke: theme.grid, width: 1 },
    ticks: { stroke: theme.axis, width: 1, size: 4 },
    font: '12px system-ui, -apple-system, "Segoe UI", sans-serif',
  };

  const opts: uPlot.Options = {
    width: Math.max(frame.clientWidth, 320),
    height: 380,
    padding: [14, 96, 0, 0],
    legend: { show: false },
    tzDate: (ts) => uPlot.tzDate(new Date(ts * 1000), "Etc/UTC"),
    scales: { x: { time: true }, y: { range: () => [Math.floor(lo - 1), Math.ceil(hi + 1)] } },
    cursor: {
      x: true,
      y: false,
      drag: { x: true, y: false, setScale: true },
      points: { size: 9, width: 2, stroke: theme.series, fill: theme.surface },
    },
    select: { show: true, left: 0, top: 0, width: 0, height: 0 },
    axes: [
      {
        ...axisBase,
        values: (_u, splits) =>
          splits.map((s, i) => {
            const step = i > 0 ? s - splits[i - 1] : (splits[1] ?? s) - s;
            const d = new Date(s * 1000);
            return step < 28 * DAY
              ? `${d.getUTCDate()} ${fmtMonthShort(s)}`
              : `${fmtMonthShort(s)} ${String(d.getUTCFullYear()).slice(2)}`;
          }),
      },
      { ...axisBase, size: 56, values: (_u, splits) => splits.map((v) => `${fmtInt(v)} °`) },
    ],
    series,
    hooks: {
      drawClear: [(u) => monthBands(u, theme), (u) => bandAndShading(u, theme)],
      draw: [(u) => endLabels(u, theme)],
      setCursor: [
        (u) => {
          const idx = u.cursor.idx;
          if (idx === null || idx === undefined || u.cursor.left === undefined || u.cursor.left < 0) {
            tooltip.hidden = true;
            return;
          }
          const p = trend.points[idx];
          const roll = rolling[idx];
          clear(tooltip);
          const rows: HTMLElement[] = [
            h("div", { class: "tt-date" }, fmtLongDay(xs[idx])),
            ttRow("Ce jour-là", fmtTemp(p.median), theme.series, true),
          ];
          if (roll !== null) rows.push(ttRow("Sur 30 jours", fmtTemp(roll), theme.series, false));
          rows.push(ttRow("Entre", `${fmtTemp(p.min)} et ${fmtTemp(p.p95)}`, null, false));
          if (trend.baseline !== null && roll !== null) {
            rows.push(ttRow("Depuis le début", fmtSigned(roll - trend.baseline), null, false));
          }
          rows.push(h("div", { class: "tt-n" }, `${fmtInt(p.n)} mesures ce jour-là`));
          tooltip.append(...rows);
          tooltip.hidden = false;
          const left = (u.cursor.left ?? 0) + u.bbox.left / devicePixelRatio;
          const flip = left > frame.clientWidth * 0.6;
          tooltip.style.left = `${flip ? left - tooltip.offsetWidth - 14 : left + 14}px`;
          tooltip.style.top = "36px";
        },
      ],
    },
  };

  const data: uPlot.AlignedData = options.trendLine
    ? [xs, median, rolling, baseline, trendLine]
    : [xs, median, rolling, baseline];
  const plot = new uPlot(opts, data, frame);

  const resize = new ResizeObserver(() => plot.setSize({ width: Math.max(frame.clientWidth, 320), height: 380 }));
  resize.observe(frame);
  return () => {
    resize.disconnect();
    plot.destroy();
  };
}

function ttRow(label: string, value: string, key: string | null, strong: boolean): HTMLElement {
  const swatch = key ? h("span", { class: "tt-key" }) : h("span", { class: "tt-key tt-key-none" });
  if (key) swatch.style.background = key;
  return h(
    "div",
    { class: "tt-row" },
    swatch,
    h("span", { class: strong ? "tt-value tt-strong" : "tt-value" }, value),
    h("span", { class: "tt-label" }, label),
  );
}
