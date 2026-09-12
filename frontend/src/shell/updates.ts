// The sidebar's Updates count, kept true: refreshed quietly after a plan
// finishes well, and on the timer Settings names, without the status line
// or a toast (those belong to the check the user asked for).

import * as api from "../api";
import { useShell } from "./store";

let inFlight: Promise<void> | null = null;

/** Re-count the updates without saying so. A failure leaves the count as it was. */
export function refreshUpdateCount(): Promise<void> {
  if (inFlight) return inFlight;
  inFlight = api
    .updates(false)
    .then((list) => useShell.getState().setUpdateCount(list.updates.length))
    .catch(() => undefined)
    .finally(() => {
      inFlight = null;
    });
  return inFlight;
}

/** Refresh every `minutes` while the window is open. Returns the stop. Zero or less means never. */
export function startUpdateTimer(minutes: number): () => void {
  if (!(minutes > 0)) return () => {};
  const id = window.setInterval(() => void refreshUpdateCount(), minutes * 60 * 1000);
  return () => window.clearInterval(id);
}
