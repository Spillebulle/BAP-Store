type Base = {
  on: boolean;
  onChange: (on: boolean) => void;
  /** The accessible name; the visible label sits to the left in the row (§7.8). */
  label: string;
};

export type ToggleProps =
  | (Base & { disabled?: false; disabledReason?: undefined })
  | (Base & { disabled: true; disabledReason: string });

/** 34 × 18 pill. On is the accent fill; nothing is written inside it. */
export function Toggle({ on, onChange, label, disabled, disabledReason }: ToggleProps) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={on}
      aria-label={label}
      className={on ? "bk-toggle on" : "bk-toggle"}
      title={disabled ? disabledReason : undefined}
      disabled={disabled}
      onClick={() => onChange(!on)}
    />
  );
}
