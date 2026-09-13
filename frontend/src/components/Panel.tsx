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
  const cls = ["bk-panel", float ? "bk-panel--float" : "", className ?? ""].filter(Boolean).join(" ");
  return (
    <section className={cls} aria-label={title}>
      <header className="bk-panel-head">
        {grip ? <span className="bk-grip" aria-hidden="true" /> : null}
        <h3 className="bk-panel-title">
          {icon}
          <span>{title}</span>
          {count !== undefined ? <span className="bk-count">{formatCount(count)}</span> : null}
        </h3>
        {commands ? <div className="bk-panel-cmds">{commands}</div> : null}
      </header>
      <div className={flush ? "bk-panel-body bk-panel-body--flush" : "bk-panel-body"}>{children}</div>
    </section>
  );
}
