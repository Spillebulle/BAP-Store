import type { ReactNode } from "react";

export type BadgeTone = "neutral" | "caution" | "good" | "critical";

interface Props {
  /** Colour means state: caution for "look at this", good, critical. Neutral for a word. */
  tone?: BadgeTone;
  icon?: ReactNode;
  title?: string;
  children: ReactNode;
}

/** 10.5 px on its palette's -bg fill, radius 3 (§7.13). */
export function Badge({ tone = "neutral", icon, title, children }: Props) {
  const cls = tone === "neutral" ? "bs-badge" : `bs-badge bs-badge--${tone}`;
  return (
    <span className={cls} title={title}>
      {icon}
      {children}
    </span>
  );
}
