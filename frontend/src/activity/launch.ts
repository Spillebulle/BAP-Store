// Opening what was installed, and the one sentence about a launcher that
// cannot list it yet.
//
// What a package would open (an entry's file name, or null for nothing to
// open) is asked of the application once per package and remembered until a
// plan finishes, because a plan is the only thing that changes the answer.
// The launcher notices (Flatpak or snapd set up after the desktop session
// started) are asked again after every plan for the same reason.
//
// After a plan that installed something, `afterPlan` says so by name with an
// Open action, and repeats any launcher notice for the formats the plan
// touched in a toast that stays: that sentence is the difference between an
// install that worked and one that looks as if it did nothing.

import { useEffect } from "react";
import { create } from "zustand";
import * as api from "../api";
import { toast } from "../components/toastStore";
import type { PackageRef, PlanStatus, SourceKind } from "../types";

function key(ref: PackageRef): string {
  return `${ref.source}:${ref.id}`;
}

function message(e: unknown): string {
  return e instanceof Error ? e.message : String(e);
}

interface LaunchState {
  /** By `source:id`: what Open opens, or null for nothing. Absent means not asked yet. */
  targets: Record<string, string | null>;
  notices: Partial<Record<SourceKind, string>>;
  /** Ask for the targets not known yet. */
  ask: (refs: PackageRef[]) => Promise<void>;
  loadNotices: () => Promise<void>;
  /** A plan finished: every answer may have changed. */
  forget: () => void;
}

/**
 * Requests on their way, by package. A second asker for the same package
 * waits for the first request instead of returning early, or a caller that
 * needs the answer (the toast after an install) could read it before it came.
 */
let inFlight = new Map<string, Promise<void>>();

export const useLaunch = create<LaunchState>((set, get) => ({
  targets: {},
  notices: {},
  ask: async (refs) => {
    const known = get().targets;
    const pending = refs.map((r) => inFlight.get(key(r))).filter((p): p is Promise<void> => p !== undefined);
    const wanted = refs.filter((r) => !(key(r) in known) && !inFlight.has(key(r)));
    let request: Promise<void> | null = null;
    if (wanted.length > 0) {
      request = (async () => {
        try {
          const answers = await api.launch_targets(wanted);
          set((s) => {
            const targets = { ...s.targets };
            wanted.forEach((r, i) => {
              targets[key(r)] = answers[i] ?? null;
            });
            return { targets };
          });
        } catch {
          // Without an answer there is simply no Open button; nothing to say.
        }
      })();
      const mine = request;
      wanted.forEach((r) => inFlight.set(key(r), mine));
      void mine.finally(() => wanted.forEach((r) => inFlight.get(key(r)) === mine && inFlight.delete(key(r))));
    }
    await Promise.all(request ? [...pending, request] : pending);
  },
  loadNotices: async () => {
    try {
      const list = await api.launcher_notices();
      set({ notices: Object.fromEntries(list) as Partial<Record<SourceKind, string>> });
    } catch {
      set({ notices: {} });
    }
  },
  forget: () => {
    inFlight = new Map();
    set({ targets: {} });
  },
}));

/** What Open would open for this package: a string, null for nothing, undefined while it is being asked. */
export function useLaunchTarget(ref: PackageRef | null | undefined): string | null | undefined {
  const refKey = ref ? key(ref) : null;
  const target = useLaunch((s) => (refKey ? s.targets[refKey] : null));
  useEffect(() => {
    if (ref && target === undefined) void useLaunch.getState().ask([ref]);
    // The reference is compared by its key; a new object for the same package asks nothing.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [refKey, target]);
  return ref ? target : null;
}

/** The same for many packages at once, in one request: for list pages. */
export function useLaunchTargets(refs: PackageRef[]): Record<string, string | null> {
  const targets = useLaunch((s) => s.targets);
  const wanted = refs.map(key).join("|");
  useEffect(() => {
    if (refs.length > 0) void useLaunch.getState().ask(refs);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [wanted, targets]);
  return targets;
}

export { key as launchKey };

/** Open a package, saying what went wrong in a toast if it did not open. */
export async function openPackage(ref: PackageRef, name: string): Promise<void> {
  try {
    await api.open_app(ref);
    toast(`Opening ${name}.`);
  } catch (e) {
    toast(message(e), "error");
  }
}

/**
 * After a plan ends well: forget what may have changed, name what was
 * installed with an Open action, and show the launcher notice for every
 * format the plan installed into or set up.
 */
export async function afterPlan(status: PlanStatus, nameOf: (ref: PackageRef) => Promise<string>): Promise<void> {
  const launch = useLaunch.getState();
  launch.forget();
  const installed = status.plan.ops.flatMap((op) => (op.op === "install" ? [op.package] : []));
  const touched = new Set<SourceKind>(
    status.plan.ops.flatMap((op) => (op.op === "setup" ? [op.source] : op.op === "install" ? [op.package.source] : [])),
  );

  await launch.loadNotices();
  const notices = useLaunch.getState().notices;

  if (installed.length > 0) {
    await launch.ask(installed);
    const targets = useLaunch.getState().targets;
    const openable = installed.find((r) => targets[key(r)]);
    const names = await Promise.all(installed.map(nameOf));
    const sentence =
      installed.length === 1 ? `${names[0]} is installed.` : `${names.slice(0, -1).join(", ")} and ${names[names.length - 1]} are installed.`;
    if (openable) {
      const name = names[installed.indexOf(openable)];
      toast(sentence, "good", { label: installed.length === 1 ? "Open" : `Open ${name}`, onClick: () => void openPackage(openable, name) });
    } else {
      toast(sentence, "good");
    }
  }

  for (const kind of touched) {
    const notice = notices[kind];
    if (notice) toast(notice, "caution", undefined, true);
  }
}

/** A plan that only installed things says so by name in `afterPlan`, so the generic "Finished." toast is left out for it. */
export function namesItsOwnOutcome(status: PlanStatus): boolean {
  return status.plan.ops.some((op) => op.op === "install");
}

