import { Figure } from "./Figure";

type Known = {
  /** 0 to 1. */
  fraction: number;
  /** What is happening, at the left. */
  message?: string | null;
  sliding?: undefined;
};

type Unknown = {
  fraction: null;
  /** With no fraction the sentence is the whole report, so it is compulsory. */
  message: string;
  /** The one permitted indeterminate animation, only where the total genuinely cannot be known. */
  sliding?: boolean;
};

export type ProgressProps = Known | Unknown;

/**
 * A 3 px rail (§7.18). A known fraction fills it in accent; an unknown one
 * draws the track empty beside a sentence, because a bar that moves over an
 * unknown is a lie. The sliding third is opt-in and rare.
 */
export function Progress(props: ProgressProps) {
  const known = props.fraction !== null;
  const pct = known ? Math.round(Math.max(0, Math.min(1, props.fraction)) * 100) : null;
  const sliding = !known && props.sliding === true;
  return (
    <div className="bk-prog-wrap" role="progressbar" aria-valuemin={0} aria-valuemax={100} aria-valuenow={pct ?? undefined} aria-valuetext={props.message ?? undefined}>
      {props.message || pct !== null ? (
        <div className="bk-prog-row">
          <span>{props.message ?? ""}</span>
          {pct !== null ? <Figure>{pct}%</Figure> : null}
        </div>
      ) : null}
      <div className={sliding ? "bk-prog bk-prog--sliding" : "bk-prog"}>
        <i className="bk-prog-fill" style={{ width: sliding ? undefined : `${pct ?? 0}%` }} />
      </div>
    </div>
  );
}
