// The confirm step of every transaction (§7.17 modal): the steps api.plan
// worked out, any sentence worth reading first, whether a password will be
// asked for, and one primary button that says what happens. Mounted once in
// App.tsx; startOps in flow.ts opens it.

import { Badge } from "../components/Badge";
import { Button } from "../components/Button";
import { Dialog } from "../components/Dialog";
import { Notice } from "../components/Notice";
import { Skeleton } from "../components/Skeleton";
import type { Step } from "../types";
import { removalSentence, summarise, useFlow, verbFor } from "./flow";

/** A step as one line for the dialog: the program and its arguments. */
function commandLine(step: Step): string {
  return [step.command.program, ...step.command.args].join(" ");
}

function StepRow({ step }: { step: Step }) {
  return (
    <li className="bs-plan-step">
      <div className="bs-plan-step-head">
        <span className="bs-plan-step-title">{step.title}</span>
        {step.needs_root ? <Badge title="This step runs as root through the helper.">needs your password</Badge> : null}
      </div>
      <div className="bs-plan-step-cmd" title={commandLine(step)}>
        {commandLine(step)}
      </div>
    </li>
  );
}

/** Rows of the same geometry while api.plan is still answering. */
function StepSkeleton() {
  return (
    <ul className="bs-plan-steps" aria-busy="true" aria-label="Working out the steps">
      {[0, 1].map((i) => (
        <li key={i} className="bs-plan-step">
          <div className="bs-plan-step-head">
            <Skeleton width={i === 0 ? "38%" : "52%"} />
          </div>
          <Skeleton width="72%" className="bs-skel--text" />
        </li>
      ))}
    </ul>
  );
}

export function FlowDialog() {
  const open = useFlow((s) => s.open);
  const title = useFlow((s) => s.title);
  const ops = useFlow((s) => s.ops);
  const preview = useFlow((s) => s.preview);
  const error = useFlow((s) => s.error);
  const starting = useFlow((s) => s.starting);
  const confirm = useFlow((s) => s.confirm);
  const cancel = useFlow((s) => s.cancel);
  if (!open) return null;

  const verb = verbFor(ops);
  const steps = preview?.plan.steps ?? [];
  const needsRoot = steps.some((s) => s.needs_root);
  const removes = removalSentence(ops, steps);
  const disabledReason = error ?? (preview ? null : "The steps are still being worked out.");

  const primary =
    disabledReason !== null ? (
      <Button kind={verb === "Remove" ? "danger" : "primary"} disabled disabledReason={disabledReason}>
        {verb}
      </Button>
    ) : (
      <Button kind={verb === "Remove" ? "danger" : "primary"} onClick={() => void confirm()}>
        {verb}
      </Button>
    );

  return (
    <Dialog
      open={open}
      title={title}
      subtitle={summarise(ops)}
      size={steps.length > 4 ? "standard" : "small"}
      onClose={cancel}
      busy={starting ? "The transaction is starting. Wait a moment." : undefined}
      actions={
        <>
          <Button kind="ghost" onClick={cancel}>
            Cancel
          </Button>
          {primary}
        </>
      }
    >
      <div className="bs-stack bs-plan">
        {removes ? <p>{removes}</p> : null}
        {error ? <Notice>{error}</Notice> : null}
        {preview?.notices.map((sentence, i) => (
          <Notice key={i}>{sentence}</Notice>
        ))}
        {preview ? (
          <ul className="bs-plan-steps" aria-label="Steps">
            {steps.map((step, i) => (
              <StepRow key={i} step={step} />
            ))}
          </ul>
        ) : error ? null : (
          <StepSkeleton />
        )}
        {preview ? (
          <p className="bs-dim bs-small">{needsRoot ? "You will be asked for your password once." : "Nothing here needs your password."}</p>
        ) : null}
      </div>
    </Dialog>
  );
}
