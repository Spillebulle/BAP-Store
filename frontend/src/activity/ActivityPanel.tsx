import { Activity, Terminal, X } from "lucide-react";
import { cancel_plan } from "../api";
import { Button } from "../components/Button";
import { IconButton } from "../components/IconButton";
import { ICON, ICON_LG } from "../components/icons";
import { Panel } from "../components/Panel";
import { Progress } from "../components/Progress";
import { toast } from "../components/toastStore";
import { plural } from "../format";
import type { Event, PlanStatus } from "../types";
import { selectActivePlan, useActivity } from "./store";

function title(plan: PlanStatus): string {
  const ops = plan.plan.ops;
  if (ops.length === 0) return "Working";
  const kinds = new Set(ops.map((o) => o.op));
  const what = plural(ops.length, "package");
  if (kinds.size === 1 && kinds.has("install")) return `Installing ${what}`;
  if (kinds.size === 1 && kinds.has("remove")) return `Removing ${what}`;
  if (kinds.has("updateall")) return "Updating everything";
  if (kinds.size === 1 && kinds.has("update")) return `Updating ${what}`;
  return `Running ${plural(ops.length, "operation")}`;
}

interface Snapshot {
  step: string | null;
  fraction: number | null;
  message: string | null;
  log: Event[];
}

/** What the latest events say: the running step, the last fraction, the last sentence. */
function snapshot(plan: PlanStatus): Snapshot {
  let step: string | null = null;
  let fraction: number | null = null;
  let message: string | null = null;
  const log: Event[] = [];
  for (const e of plan.events) {
    switch (e.event) {
      case "step_started":
        step = e.title;
        fraction = null;
        message = null;
        break;
      case "progress":
        fraction = e.fraction;
        message = e.message ?? message;
        break;
      case "log":
        log.push(e);
        break;
      case "step_finished":
        if (e.message) log.push({ event: "log", plan: e.plan, step: e.step, line: e.message, stderr: !e.ok });
        break;
      case "auth_required":
        message = "Waiting for authorisation. Enter your password in the system dialog.";
        break;
      case "plan_finished":
        message = e.message;
        break;
      case "plan_started":
        break;
    }
  }
  return { step, fraction, message, log };
}

/** The floating panel at the bottom right while a transaction runs: step, rail, log toggle. */
export function ActivityPanel() {
  const plan = useActivity(selectActivePlan);
  const showLog = useActivity((s) => s.showLog);
  const toggleLog = useActivity((s) => s.toggleLog);
  const dismiss = useActivity((s) => s.dismiss);
  if (!plan) return null;

  const { step, fraction, message, log } = snapshot(plan);
  const live = plan.state === "pending" || plan.state === "authorising" || plan.state === "running";
  const sentence = plan.state === "authorising" ? "Waiting for authorisation. Enter your password in the system dialog." : (message ?? "Starting.");

  const cancel = async () => {
    try {
      await cancel_plan(plan.plan.id);
    } catch (e) {
      toast(e instanceof Error ? e.message : String(e), "error");
    }
  };

  return (
    <Panel
      float
      className="bs-activity"
      title={title(plan)}
      icon={<Activity {...ICON_LG} aria-hidden="true" />}
      commands={
        <>
          <IconButton size="sm" label={showLog ? "Hide the log" : "Show the log"} active={showLog} icon={<Terminal {...ICON} aria-hidden="true" />} onClick={toggleLog} />
          {live ? null : <IconButton size="sm" label="Dismiss" icon={<X {...ICON} aria-hidden="true" />} onClick={() => dismiss(plan.plan.id)} />}
        </>
      }
    >
      <div className="bs-activity-body">
        {step ? <div className="bs-small">{step}</div> : null}
        {fraction !== null ? <Progress fraction={fraction} message={message} /> : <Progress fraction={null} message={sentence} />}
        {showLog ? (
          <pre className="bs-activity-log" aria-label="Log">
            {log.length === 0 ? "Nothing has been written yet." : null}
            {log.map((e, i) =>
              e.event === "log" ? (
                <span key={i} className={e.stderr ? "err" : undefined}>
                  {e.line}
                  {"\n"}
                </span>
              ) : null,
            )}
          </pre>
        ) : null}
        {live ? (
          <div className="bs-btn-group" style={{ justifyContent: "flex-end" }}>
            <Button kind="ghost" onClick={cancel}>
              Cancel
            </Button>
          </div>
        ) : null}
      </div>
    </Panel>
  );
}
