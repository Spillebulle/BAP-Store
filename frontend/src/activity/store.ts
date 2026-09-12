// Every transaction the page has seen, by plan id, with its events appended
// as they arrive. One subscription to `transaction://event` feeds it; the
// panel and the status bar read from it. A plan is kept after it finishes so
// its log can still be read, until the user dismisses it.

import { create } from "zustand";
import { onTransactionEvent } from "../api";
import { toast } from "../components/toastStore";
import type { Event, PlanStatus } from "../types";

interface ActivityState {
  plans: Record<string, PlanStatus>;
  /** Plan ids, oldest first. */
  order: string[];
  showLog: boolean;
  toggleLog: () => void;
  /** What run_plan returned: the plan's steps, before or after its first event. */
  track: (status: PlanStatus) => void;
  apply: (event: Event) => void;
  dismiss: (id: string) => void;
}

/** An event can arrive before run_plan has returned; a stub holds it until the plan is known. */
function stub(id: string): PlanStatus {
  return {
    plan: { id, ops: [], steps: [] },
    state: "pending",
    events: [],
    started: Math.floor(Date.now() / 1000),
  };
}

export const useActivity = create<ActivityState>((set, get) => ({
  plans: {},
  order: [],
  showLog: false,
  toggleLog: () => set((s) => ({ showLog: !s.showLog })),

  track: (status) => {
    const { plans, order } = get();
    const existing = plans[status.plan.id];
    const merged: PlanStatus = existing
      ? { ...existing, plan: status.plan, started: status.started }
      : { ...status, events: [...status.events] };
    set({
      plans: { ...plans, [status.plan.id]: merged },
      order: existing ? order : [...order, status.plan.id],
    });
  },

  apply: (event) => {
    const { plans, order } = get();
    const current = plans[event.plan] ?? stub(event.plan);
    const next: PlanStatus = { ...current, events: [...current.events, event] };
    switch (event.event) {
      case "auth_required":
        next.state = "authorising";
        break;
      case "plan_started":
      case "step_started":
      case "progress":
      case "log":
      case "step_finished":
        if (next.state !== "authorising" || event.event === "step_started") next.state = "running";
        break;
      case "plan_finished":
        next.state = event.ok ? "done" : event.message.startsWith("Cancelled") ? "cancelled" : "failed";
        toast(event.message, event.ok ? "good" : "error");
        break;
    }
    set({
      plans: { ...plans, [event.plan]: next },
      order: plans[event.plan] ? order : [...order, event.plan],
    });
  },

  dismiss: (id) => {
    const { plans, order } = get();
    const rest = { ...plans };
    delete rest[id];
    set({ plans: rest, order: order.filter((o) => o !== id) });
  },
}));

const LIVE = new Set(["pending", "authorising", "running"]);

/** The plan the panel shows: the newest one still running, else the newest failure awaiting a look. */
export function selectActivePlan(state: ActivityState): PlanStatus | null {
  for (let i = state.order.length - 1; i >= 0; i -= 1) {
    const p = state.plans[state.order[i]];
    if (p && LIVE.has(p.state)) return p;
  }
  for (let i = state.order.length - 1; i >= 0; i -= 1) {
    const p = state.plans[state.order[i]];
    if (p && p.state === "failed") return p;
  }
  return null;
}

let unsubscribe: (() => void) | null = null;

/** Subscribe once for the life of the page. Returns the unsubscribe for the caller that started it. */
export function subscribeActivity(): () => void {
  if (unsubscribe) return () => {};
  unsubscribe = onTransactionEvent((event) => useActivity.getState().apply(event));
  return () => {
    unsubscribe?.();
    unsubscribe = null;
  };
}
