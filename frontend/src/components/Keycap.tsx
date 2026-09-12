import type { ReactNode } from "react";

/** A shortcut in mono on a control cap (§7.13). A clashing key wears the caution palette. */
export function Keycap({ children, clash, title }: { children: ReactNode; clash?: boolean; title?: string }) {
  return (
    <kbd className={clash ? "bs-key bs-key--clash" : "bs-key"} title={title}>
      {children}
    </kbd>
  );
}
