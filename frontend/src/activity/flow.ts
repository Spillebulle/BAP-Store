// The one way a page starts a transaction. Pages call `startOps` with the
// operations they want; what happens next (a confirm step showing the
// plan's steps and notices, then the run, tracked in the activity panel) is
// this module's business, so every page starts an install the same way.
//
// This is the minimal form: run at once, track the result. The confirm
// dialog replaces the body without changing the signature.

import * as api from "../api";
import { toast } from "../components/toastStore";
import type { Op } from "../types";
import { useActivity } from "./store";

/** Start a plan for these operations; the returned id is the plan's, or null when it did not start. */
export async function startOps(ops: Op[], _title: string): Promise<string | null> {
  try {
    const id = await api.run_plan(ops);
    const statuses = await api.active_plans();
    const status = statuses.find((s) => s.plan.id === id);
    if (status) useActivity.getState().track(status);
    return id;
  } catch (e) {
    toast(e instanceof Error ? e.message : String(e), "error");
    return null;
  }
}
