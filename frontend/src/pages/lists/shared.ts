// What the two list pages share: the options of a sources filter (an
// unavailable source is disabled with its reason, never dropped), the key
// that names one package, the disabled-with-a-reason props a control takes,
// a version comparison for the self-update notice, and a way to notice a
// plan finishing so a list re-asks the sources rather than showing what was
// true before the transaction.

import { useActivity } from "../../activity/store";
import type { DropdownOption } from "../../components";
import { sourceLabel, type PackageRef, type SourceKind, type SourceStatus } from "../../types";

export function refKey(ref: PackageRef): string {
  return `${ref.source}:${ref.id}`;
}

export function sourceOptions(sources: SourceStatus[]): DropdownOption<SourceKind>[] {
  return sources.map((s) => ({
    value: s.kind,
    label: sourceLabel(s.kind),
    disabled: !s.available,
    disabledReason: s.available ? undefined : (s.reason ?? "This source is not available on this machine."),
  }));
}

export function availableKinds(sources: SourceStatus[]): SourceKind[] {
  return sources.filter((s) => s.available).map((s) => s.kind);
}

/** "pacman, AUR and Flatpak". */
export function joinNames(names: string[]): string {
  if (names.length <= 1) return names.join("");
  return `${names.slice(0, -1).join(", ")} and ${names[names.length - 1]}`;
}

/** The props that disable a control and say why (§12), for a condition known at render. */
export function off(when: boolean, reason: string): { disabled: true; disabledReason: string } | { disabled?: false; disabledReason?: undefined } {
  return when ? { disabled: true, disabledReason: reason } : {};
}

/** Whether `latest` is newer than `current`: numeric fields compare as numbers, the rest as text. */
export function isNewer(latest: string, current: string): boolean {
  const split = (v: string) => v.trim().replace(/^v/i, "").split(/[.\-+_]/);
  const a = split(latest);
  const b = split(current);
  for (let i = 0; i < Math.max(a.length, b.length); i += 1) {
    const x = a[i] ?? "0";
    const y = b[i] ?? "0";
    const nx = Number(x);
    const ny = Number(y);
    if (Number.isFinite(nx) && Number.isFinite(ny)) {
      if (nx !== ny) return nx > ny;
    } else if (x !== y) {
      return x.localeCompare(y) > 0;
    }
  }
  return false;
}

/** Calls back each time a plan reaches "done". Returns the unsubscribe. */
export function onPlanDone(callback: () => void): () => void {
  return useActivity.subscribe((state, previous) => {
    for (const id of state.order) {
      if (state.plans[id]?.state === "done" && previous.plans[id]?.state !== "done") {
        callback();
        return;
      }
    }
  });
}

/** A sentence for a source that failed: the source's name, then what it said. */
export function failedSentence(kind: SourceKind, sentence: string, what: string): string {
  const said = sentence.trim();
  return `${sourceLabel(kind)} could not be ${what}. ${said.endsWith(".") ? said : `${said}.`}`;
}

export function message(e: unknown): string {
  return e instanceof Error ? e.message : String(e);
}
