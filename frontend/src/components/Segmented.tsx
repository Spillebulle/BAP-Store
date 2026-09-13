import type { ReactNode } from "react";

export interface SegmentOption<T extends string> {
  value: T;
  label?: string;
  icon?: ReactNode;
  /** Required when the segment is icon-only: an icon without a label has a tooltip. */
  title?: string;
  disabled?: boolean;
  disabledReason?: string;
}

interface Props<T extends string> {
  /** The group's accessible name: what is being chosen. */
  name: string;
  options: SegmentOption<T>[];
  value: T;
  onChange: (value: T) => void;
}

/** Two to five exclusive short options (§7.9). Selected is the control fill, never the accent. */
export function Segmented<T extends string>({ name, options, value, onChange }: Props<T>) {
  return (
    <div className="bk-seg" role="radiogroup" aria-label={name}>
      {options.map((o) => (
        <button
          key={o.value}
          type="button"
          role="radio"
          aria-checked={o.value === value}
          className={o.value === value ? "on" : undefined}
          title={o.disabled ? o.disabledReason : (o.title ?? (o.label ? undefined : o.value))}
          aria-label={o.label ? undefined : (o.title ?? o.value)}
          disabled={o.disabled}
          onClick={() => onChange(o.value)}
        >
          {o.icon}
          {o.label}
        </button>
      ))}
    </div>
  );
}
