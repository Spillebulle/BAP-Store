import { X } from "lucide-react";
import { useEffect, useId, useRef, useState, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { IconButton } from "./IconButton";
import { ICON } from "./icons";

export type DialogSize = "small" | "standard" | "large";

interface Props {
  open: boolean;
  title: string;
  /** One line of muted text under the title. */
  subtitle?: string;
  /** small 430, standard 760, large 1000 × 640; all capped at 92 % of the viewport. */
  size?: DialogSize;
  onClose: () => void;
  /** A note at the left of the footer in dim ("Saved to ..."). */
  note?: ReactNode;
  /** Buttons at the right of the footer, primary rightmost. */
  actions?: ReactNode;
  /** While set, the dialog refuses to close and says this. */
  busy?: string;
  children: ReactNode;
}

/** A modal (§7.17): chrome on a dimmed page, one fixed size whatever page is in it, Escape closes. */
export function Dialog({ open, title, subtitle, size = "small", onClose, note, actions, busy, children }: Props) {
  const id = useId();
  const box = useRef<HTMLDivElement>(null);
  const [refused, setRefused] = useState<string | null>(null);

  const tryClose = () => {
    if (busy) {
      setRefused(busy);
      return;
    }
    onClose();
  };

  useEffect(() => {
    if (!open) {
      setRefused(null);
      return;
    }
    const previous = document.activeElement as HTMLElement | null;
    box.current?.focus();
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      // Escape inside a field first drops the field's edit; the field handles that itself.
      if (e.defaultPrevented) return;
      e.preventDefault();
      tryClose();
    };
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("keydown", onKey);
      previous?.focus();
    };
    // tryClose closes over busy/onClose; re-binding on every change would refocus the box.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open, busy]);

  if (!open) return null;

  const cls = size === "small" ? "bk-dialog" : `bk-dialog bk-dialog--${size}`;
  return createPortal(
    <div className="bk-dimmer" onPointerDown={(e) => e.target === e.currentTarget && tryClose()}>
      <div ref={box} className={cls} role="dialog" aria-modal="true" aria-labelledby={`${id}-title`} tabIndex={-1}>
        <div className="bk-dialog-head">
          <div>
            <h2 id={`${id}-title`}>{title}</h2>
            {subtitle ? <p>{subtitle}</p> : null}
          </div>
          <IconButton label="Close" icon={<X {...ICON} aria-hidden="true" />} onClick={tryClose} />
        </div>
        <div className="bk-dialog-body">{children}</div>
        {note !== undefined || actions || refused ? (
          <div className="bk-dialog-foot">
            <span>{refused ?? note}</span>
            {actions ? <div className="bk-btn-group">{actions}</div> : null}
          </div>
        ) : null}
      </div>
    </div>,
    document.body,
  );
}
