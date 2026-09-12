import type { ReactNode } from "react";
import { formatCount } from "../format";

interface Props {
  title: string;
  /** A 20 px icon before the title. */
  icon?: ReactNode;
  count?: number;
  /** Right-aligned 20 px icon buttons, at most four. */
  commands?: ReactNode;
  /** The drag grip of a dockable module. Off for a card that is a region. */
  grip?: boolean;
  /** Floating: chrome at 96 %, blur, the float shadow. */
  float?: boolean;
  /** No body padding: a list that runs edge to edge. */
  flush?: boolean;
  children: ReactNode;
  className?: string;
}

/** A titled region (§7.5): 32 px header that never scrolls away, body with 12 px padding. */
export function Panel({ title, icon, count, commands, grip, float, flush, children, className }: Props) {
  const cls = ["bs-panel", float ? "bs-panel--float" : "", className ?? ""].filter(Boolean).join(" ");
  return (
    <section className={cls} aria-label={title}>
      <header className="bs-panel-head">
        {grip ? <span className="bs-grip" aria-hidden="true" /> : null}
        <h3 className="bs-panel-title">
          {icon}
          <span>{title}</span>
          {count !== undefined ? <span className="bs-count">{formatCount(count)}</span> : null}
        </h3>
        {commands ? <div className="bs-panel-cmds">{commands}</div> : null}
      </header>
      <div className={flush ? "bs-panel-body bs-panel-body--flush" : "bs-panel-body"}>{children}</div>
    </section>
  );
}
