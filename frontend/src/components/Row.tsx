import type { KeyboardEvent, MouseEvent, ReactNode } from "react";
import type { App, Edition } from "../types";
import { AppIcon } from "./AppIcon";
import { SourceBadge } from "./SourceBadge";

interface RowProps {
  /** The selected-row look: control fill and strong text. */
  selected?: boolean;
  /** The active thing among selected ones: accent-dim edge and the 7 % wash. */
  active?: boolean;
  /** A text-only row is 20 px; one with a picture is 26. */
  plain?: boolean;
  onClick?: () => void;
  /** An icon or a thumbnail at the left. */
  leading?: ReactNode;
  children: ReactNode;
  /** A figure or a state at the right, in dim mono. */
  trailing?: ReactNode;
  className?: string;
  title?: string;
}

function activate(onClick: (() => void) | undefined) {
  return (e: KeyboardEvent<HTMLDivElement>) => {
    if (!onClick) return;
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      onClick();
    }
  };
}

/** A list row (§7.16). Clickable rows are buttons in behaviour but divs in markup, so they can hold a button. */
export function Row({ selected, active, plain, onClick, leading, children, trailing, className, title }: RowProps) {
  const classes = [
    "bs-row",
    plain ? "bs-row--plain" : "",
    onClick ? "clickable" : "",
    selected ? "sel" : "",
    active ? "act" : "",
    className ?? "",
  ]
    .filter(Boolean)
    .join(" ");
  return (
    <div
      className={classes}
      role={onClick ? "button" : undefined}
      tabIndex={onClick ? 0 : undefined}
      aria-pressed={onClick && selected !== undefined ? selected : undefined}
      title={title}
      onClick={onClick}
      onKeyDown={activate(onClick)}
    >
      {leading}
      <span className="bs-row-label">{children}</span>
      {trailing !== undefined && trailing !== null ? <span className="bs-row-trail">{trailing}</span> : null}
    </div>
  );
}

interface AppRowProps {
  app: App;
  onOpen?: (app: App) => void;
  selected?: boolean;
  /** A checkbox at a fixed x, before the icon (the Updates page). */
  check?: ReactNode;
  /** The trailing figure: a version, a size, a count. */
  figure?: ReactNode;
  /** The one action at the right: a button. Clicks on it do not open the row. */
  action?: ReactNode;
  /** A second line instead of the summary. */
  sub?: ReactNode;
}

/** One badge per source; the installed edition's badge carries the check. */
function editionBadges(editions: Edition[]) {
  const seen = new Map<string, Edition>();
  for (const e of editions) {
    const prior = seen.get(e.package.source);
    if (!prior || (!prior.package.installed && e.package.installed)) seen.set(e.package.source, e);
  }
  return [...seen.values()].map((e) => (
    <SourceBadge key={e.package.source} source={e.package.source} installed={e.package.installed} repo={e.package.repo} />
  ));
}

/**
 * The application row: 44 px, sized by --app-icon-row plus 12. Two text
 * lines, a badge per source, a trailing figure and one action.
 */
export function AppRow({ app, onOpen, selected, check, figure, action, sub }: AppRowProps) {
  const open = onOpen ? () => onOpen(app) : undefined;
  const stop = (e: MouseEvent) => e.stopPropagation();
  const classes = ["bs-row", "bs-row--app", open ? "clickable" : "", selected ? "sel" : ""].filter(Boolean).join(" ");
  return (
    <div
      className={classes}
      role={open ? "button" : undefined}
      tabIndex={open ? 0 : undefined}
      onClick={open}
      onKeyDown={activate(open)}
    >
      {check ? (
        <span className="bs-row-check" onClick={stop}>
          {check}
        </span>
      ) : null}
      <AppIcon picture={app.icon} name={app.name} size="row" />
      <div className="bs-row-text">
        <div className="bs-row-name">
          <span className="bs-row-label">{app.name}</span>
          <span className="bs-row-badges">{editionBadges(app.editions)}</span>
        </div>
        <div className="bs-row-sub">{sub ?? app.summary ?? app.developer ?? ""}</div>
      </div>
      {figure !== undefined && figure !== null ? <span className="bs-row-figure">{figure}</span> : null}
      {action ? (
        <div className="bs-row-action" onClick={stop} onKeyDown={(e) => e.stopPropagation()}>
          {action}
        </div>
      ) : null}
    </div>
  );
}
