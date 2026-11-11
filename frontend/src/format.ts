// Numbers and dates as the CHART prints them (axes, tooltip). Everything else the page shows arrives
// already written by the backend.

const number1 = new Intl.NumberFormat("fr-FR", { minimumFractionDigits: 1, maximumFractionDigits: 1 });

export const fmtTemp = (v: number): string => `${number1.format(v)} °C`;

export const fmtSigned = (v: number): string => `${v > 0 ? "+" : v < 0 ? "−" : ""}${number1.format(Math.abs(v))} °C`;

/** Parses "YYYY-MM-DD" as a UTC date (the daily aggregates have no time zone). */
export function parseDay(day: string): number {
  const [y, m, d] = day.split("-").map(Number);
  return Date.UTC(y, m - 1, d) / 1000;
}

const longDay = new Intl.DateTimeFormat("fr-FR", {
  weekday: "short", day: "numeric", month: "long", year: "numeric", timeZone: "UTC",
});
export const fmtLongDay = (unix: number): string => longDay.format(new Date(unix * 1000));

const monthName = new Intl.DateTimeFormat("fr-FR", { month: "short", timeZone: "UTC" });
export const fmtMonthShort = (unix: number): string => monthName.format(new Date(unix * 1000));
