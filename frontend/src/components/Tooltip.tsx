import type { ReactNode } from "react";

interface Props {
  /** A sentence with a full stop, or a short phrase for a control's name. */
  text: string;
  children: ReactNode;
}

/**
 * The native title tooltip: it appears after the platform's own delay, never
 * on a delay for keyboard focus, and it needs no positioning code. A painted
 * popover tooltip (§7.17) can replace this without touching call sites.
 */
export function Tooltip({ text, children }: Props) {
  return (
    <span className="bs-inline" style={{ display: "inline-flex" }} title={text}>
      {children}
    </span>
  );
}
