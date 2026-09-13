// The one sentence about a newer Brokey, above the page (§7.17 notice):
// the version, the remedy chosen for how this copy was installed (§18.3),
// and the actions that can actually work here. "Later" hides it for the
// session; Settings can turn the check off altogether.

import { useState } from "react";
import { trackPlan } from "../activity/flow";
import { markSelfUpdate } from "../activity/launch";
import * as api from "../api";
import { Button } from "../components/Button";
import { Notice } from "../components/Notice";
import { toast } from "../components/toastStore";
import type { SelfUpdate } from "../types";
import { useShell } from "./store";

/** Whether `latest` is a newer version than `current`, by dotted number; anything unreadable is not newer. */
export function isNewer(latest: string, current: string): boolean {
  const parse = (v: string) => v.replace(/^v/, "").split(/[.-]/).map((p) => Number.parseInt(p, 10));
  const a = parse(latest);
  const b = parse(current);
  if (a.some(Number.isNaN) || b.some(Number.isNaN)) return false;
  for (let i = 0; i < Math.max(a.length, b.length); i += 1) {
    const x = a[i] ?? 0;
    const y = b[i] ?? 0;
    if (x !== y) return x > y;
  }
  return false;
}

function canApply(update: SelfUpdate): boolean {
  return update.remedy?.kind === "install_asset" || update.remedy?.kind === "replace_file";
}

export function SelfUpdateNotice() {
  const update = useShell((s) => s.selfUpdate);
  const hidden = useShell((s) => s.selfUpdateHidden);
  const hide = useShell((s) => s.hideSelfUpdate);
  const go = useShell((s) => s.go);
  const view = useShell((s) => s.view);
  const [applying, setApplying] = useState(false);
  // The Updates page draws its own, more specific notice (it can name the
  // row); two sentences about the same release on one screen is one too many.
  if (view === "updates") return null;
  if (hidden || !update || !update.latest || !isNewer(update.latest.version, update.current)) return null;
  const { latest, remedy } = update;

  const apply = async () => {
    setApplying(true);
    try {
      const status = await api.self_update_apply();
      markSelfUpdate(status.plan.id);
      trackPlan(status);
      hide();
    } catch (e) {
      toast(e instanceof Error ? e.message : String(e), "error");
    } finally {
      setApplying(false);
    }
  };

  const first =
    remedy?.kind === "updates_page" ? (
      <Button kind="ghost" onClick={() => go("updates")}>
        Show updates
      </Button>
    ) : canApply(update) ? (
      applying ? (
        <Button kind="ghost" disabled disabledReason="The update is being started.">
          Update Brokey
        </Button>
      ) : (
        <Button kind="ghost" onClick={() => void apply()}>
          Update Brokey
        </Button>
      )
    ) : null;

  return (
    <div className="bk-content-notice">
      <Notice
        actions={
          <>
            {first}
            <Button kind="ghost" onClick={() => void api.openUrl(latest.url)}>
              Release notes
            </Button>
            <Button kind="ghost" onClick={hide}>
              Later
            </Button>
          </>
        }
      >
        Brokey {latest.version} is available.{remedy ? ` ${remedy.sentence}` : ""}
      </Notice>
    </div>
  );
}
