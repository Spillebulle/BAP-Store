import type { ReactNode } from "react";
import { formatBytes, formatCount, formatTime, formatTimestamp } from "../format";

export { formatBytes, formatCount, formatDate, formatTime, formatTimestamp, plural } from "../format";

interface Props {
  children: ReactNode;
  /** The full stamp, the exact byte count: whatever the short figure leaves out. */
  title?: string;
  className?: string;
}

/** Every number that is read as a value: mono, tabular, 10.5 px (§4). */
export function Figure({ children, title, className }: Props) {
  return (
    <span className={className ? `bs-figure ${className}` : "bs-figure"} title={title}>
      {children}
    </span>
  );
}

/** "1.5 GB", with the exact count in the tooltip. */
export function Bytes({ value, className }: { value: number | null | undefined; className?: string }) {
  if (value === null || value === undefined) return null;
  return (
    <Figure title={`${formatCount(value)} bytes`} className={className}>
      {formatBytes(value)}
    </Figure>
  );
}

/** "3 min ago" under a day, else the date; the full timestamp in the tooltip. */
export function When({ value, className }: { value: number | null | undefined; className?: string }) {
  if (value === null || value === undefined) return null;
  return (
    <Figure title={formatTimestamp(value)} className={className}>
      {formatTime(value)}
    </Figure>
  );
}

/** "100 548". */
export function Count({ value, className }: { value: number | null | undefined; className?: string }) {
  if (value === null || value === undefined) return null;
  return <Figure className={className}>{formatCount(value)}</Figure>;
}
