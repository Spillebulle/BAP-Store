// Figures, formatted the one way the house does it (§12): units with a
// space, thin thousands separation with a plain space, relative time under a
// day and a date after that.

const THIN = " ";

/** "1.5 GB", "245 MB", "12 kB". Decimal units, because that is what every source's own page shows. */
export function formatBytes(bytes: number | null | undefined): string {
  if (bytes === null || bytes === undefined || !Number.isFinite(bytes)) return "";
  const units = ["B", "kB", "MB", "GB", "TB"];
  let value = bytes;
  let unit = 0;
  while (value >= 1000 && unit < units.length - 1) {
    value /= 1000;
    unit += 1;
  }
  const digits = unit === 0 ? 0 : value < 10 ? 1 : 0;
  return `${value.toFixed(digits)}${THIN}${units[unit]}`;
}

/** "100 548": groups of three separated by a plain space, never a comma. */
export function formatCount(n: number | null | undefined): string {
  if (n === null || n === undefined || !Number.isFinite(n)) return "";
  const whole = Math.trunc(Math.abs(n)).toString();
  const grouped = whole.replace(/\B(?=(\d{3})+(?!\d))/g, THIN);
  return n < 0 ? `-${grouped}` : grouped;
}

/** Relative under a day ("3 min ago"), else the date. `now` is injectable for tests and fixtures. */
export function formatTime(unixSeconds: number | null | undefined, now: number = Date.now() / 1000): string {
  if (unixSeconds === null || unixSeconds === undefined || !Number.isFinite(unixSeconds)) return "";
  const delta = now - unixSeconds;
  if (delta >= 0 && delta < 86400) {
    if (delta < 60) return "just now";
    if (delta < 3600) return `${Math.floor(delta / 60)}${THIN}min ago`;
    return `${Math.floor(delta / 3600)}${THIN}h ago`;
  }
  return formatDate(unixSeconds);
}

/** "12 Sep 2026". */
export function formatDate(unixSeconds: number): string {
  return new Date(unixSeconds * 1000).toLocaleDateString("en-GB", {
    day: "numeric",
    month: "short",
    year: "numeric",
  });
}

/** The full stamp for a tooltip: "12 Sep 2026, 14:03". */
export function formatTimestamp(unixSeconds: number): string {
  return new Date(unixSeconds * 1000).toLocaleString("en-GB", {
    day: "numeric",
    month: "short",
    year: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

/** "3 packages", "1 package". */
export function plural(n: number, one: string, many: string = `${one}s`): string {
  return `${formatCount(n)}${THIN}${n === 1 ? one : many}`;
}

/** Whether a fact's value reads as a figure and so is set in mono. */
export function looksNumeric(value: string): boolean {
  return /^[\d.,\s]+(\s?[A-Za-z%×]+)?$/.test(value.trim()) || /^v?\d+(\.\d+)+/.test(value.trim());
}
