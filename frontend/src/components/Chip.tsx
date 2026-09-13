import { X } from "lucide-react";
import type { ReactNode } from "react";
import { ICON_SM } from "./icons";

interface Props {
  /** The read-only label in dim; the value beside it in mono. */
  label?: string;
  value?: ReactNode;
  /** A chip standing for a selection made elsewhere can be removed; the trigger keeps its summary. */
  onRemove?: () => void;
  children?: ReactNode;
}

/** A read-only figure on a dock pill (§7.2). A chip is not a control and never opens. */
export function Chip({ label, value, onRemove, children }: Props) {
  const name = label ?? (typeof children === "string" ? children : "this");
  return (
    <span className="bk-chip">
      {label ? <span className="bk-chip-label">{label}</span> : null}
      {value !== undefined ? <span className="bk-chip-value">{value}</span> : null}
      {children}
      {onRemove ? (
        <button type="button" className="bk-chip-x" title={`Remove ${name}`} aria-label={`Remove ${name}`} onClick={onRemove}>
          <X {...ICON_SM} aria-hidden="true" />
        </button>
      ) : null}
    </span>
  );
}
