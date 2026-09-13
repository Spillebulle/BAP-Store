import { TriangleAlert } from "lucide-react";
import type { ReactNode } from "react";
import { ICON } from "./icons";

interface Props {
  /** A sentence in text. */
  children: ReactNode;
  /** Ghost buttons at the right. */
  actions?: ReactNode;
}

/** The caution box inside a panel or a dialog (§7.17). It informs; it never blocks. */
export function Notice({ children, actions }: Props) {
  return (
    <div className="bk-notice" role="status">
      <TriangleAlert {...ICON} aria-hidden="true" />
      <div className="bk-notice-text">{children}</div>
      {actions ? <div className="bk-notice-actions">{actions}</div> : null}
    </div>
  );
}
