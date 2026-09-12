import type { ReactNode } from "react";

interface Props {
  /** A 24 px icon, drawn in line-dashed. Pass a lucide component with ICON_EMPTY spread on it. */
  icon?: ReactNode;
  /** One sentence: what would be here and how it gets here. */
  children: ReactNode;
  /** A secondary button, if there is a way to get something here. */
  action?: ReactNode;
  /** Take the whole region. */
  fill?: boolean;
}

/** Centred in the region, one sentence in dim, never an illustration (§7.19). */
export function EmptyState({ icon, children, action, fill }: Props) {
  return (
    <div className={fill ? "bs-empty bs-empty--fill" : "bs-empty"}>
      {icon}
      <p>{children}</p>
      {action}
    </div>
  );
}
