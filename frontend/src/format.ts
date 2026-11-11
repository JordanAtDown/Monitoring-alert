const number1 = new Intl.NumberFormat("fr-FR", { minimumFractionDigits: 1, maximumFractionDigits: 1 });
const number0 = new Intl.NumberFormat("fr-FR", { maximumFractionDigits: 0 });

export const fmtTemp = (v: number): string => `${number1.format(v)} °C`;

export const fmtSigned = (v: number, unit = "°C"): string =>
  `${v > 0 ? "+" : v < 0 ? "−" : ""}${number1.format(Math.abs(v))} ${unit}`;

export const fmtInt = (v: number): string => number0.format(v);

export function fmtBytes(bytes: number): string {
  if (bytes < 1024 * 1024) return `${number1.format(bytes / 1024)} Ko`;
  if (bytes < 1024 ** 3) return `${number1.format(bytes / 1024 ** 2)} Mo`;
  return `${number1.format(bytes / 1024 ** 3)} Go`;
}

/** Parses "YYYY-MM-DD" as a UTC date (the daily aggregates have no time zone). */
export function parseDay(day: string): number {
  const [y, m, d] = day.split("-").map(Number);
  return Date.UTC(y, m - 1, d) / 1000;
}

const longDay = new Intl.DateTimeFormat("fr-FR", {
  weekday: "short", day: "numeric", month: "long", year: "numeric", timeZone: "UTC",
});
export const fmtLongDay = (unix: number): string => longDay.format(new Date(unix * 1000));

const shortDay = new Intl.DateTimeFormat("fr-FR", { day: "numeric", month: "long", year: "numeric", timeZone: "UTC" });
export const fmtDay = (day: string): string => shortDay.format(new Date(parseDay(day) * 1000));

const monthName = new Intl.DateTimeFormat("fr-FR", { month: "short", timeZone: "UTC" });
const monthLong = new Intl.DateTimeFormat("fr-FR", { month: "long", year: "numeric", timeZone: "UTC" });
export const fmtMonthShort = (unix: number): string => monthName.format(new Date(unix * 1000));
export const fmtMonthLong = (unix: number): string => monthLong.format(new Date(unix * 1000));

/** Parses the server's local timestamps ("YYYY-MM-DDTHH:MM:SS", no zone) in the browser's zone. */
export function parseLocal(ts: string): Date {
  return new Date(ts);
}

export function fmtAgo(from: string | null, now: string): string {
  if (!from) return "jamais";
  const seconds = Math.max(0, (parseLocal(now).getTime() - parseLocal(from).getTime()) / 1000);
  if (seconds < 90) return "à l'instant";
  const minutes = Math.round(seconds / 60);
  if (minutes < 90) return `il y a ${minutes} min`;
  const hours = Math.round(minutes / 60);
  if (hours < 36) return `il y a ${hours} h`;
  return `il y a ${Math.round(hours / 24)} j`;
}
