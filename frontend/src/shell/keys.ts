// The window's few keyboard shortcuts. Ctrl+F or "/" goes to Search (the
// Search page focuses its own field on mount), Ctrl+, opens Settings, and
// Backspace goes back when nothing is being typed and no dialog is open.
// Escape belongs to the dialog and the menus themselves.

import { useEffect } from "react";
import { useShell } from "./store";

/** Whether a key press belongs to something that takes text. */
function typing(): boolean {
  const el = document.activeElement;
  if (!el || el === document.body) return false;
  const tag = el.tagName;
  if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT") return true;
  return (el as HTMLElement).isContentEditable;
}

function dialogOpen(): boolean {
  return document.querySelector(".bs-dimmer") !== null;
}

export function useShortcuts(): void {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.defaultPrevented) return;
      const { go, back, history } = useShell.getState();
      const ctrl = e.ctrlKey || e.metaKey;
      if (ctrl && !e.shiftKey && !e.altKey && e.key.toLowerCase() === "f") {
        e.preventDefault();
        go("search");
        return;
      }
      if (ctrl && !e.shiftKey && !e.altKey && e.key === ",") {
        e.preventDefault();
        go("settings");
        return;
      }
      if (ctrl || e.altKey) return;
      if (e.key === "/" && !typing() && !dialogOpen()) {
        e.preventDefault();
        go("search");
        return;
      }
      if (e.key === "Backspace" && !typing() && !dialogOpen() && history.length > 0) {
        e.preventDefault();
        back();
      }
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, []);
}
