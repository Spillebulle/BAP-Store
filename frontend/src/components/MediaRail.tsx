import type { ReactNode } from "react";
import { formatCount } from "../format";

interface Props {
  title: string;
  count?: number;
  /** "Show all" at the right of the title, never on the cards. */
  onShowAll?: () => void;
  children: ReactNode;
}

/** A row of one kind of thing as pictures, scrolling inside its own container (§10). */
export function MediaRail({ title, count, onShowAll, children }: Props) {
  return (
    <section className="bs-rail" aria-label={title}>
      <div className="bs-rail-head">
        <h3 className="bs-section-title">
          {title}
          {count !== undefined ? <span className="bs-count">{formatCount(count)}</span> : null}
        </h3>
        {onShowAll ? (
          <button type="button" className="bs-rail-all" onClick={onShowAll}>
            Show all
          </button>
        ) : null}
      </div>
      <div className="bs-rail-track">{children}</div>
    </section>
  );
}
