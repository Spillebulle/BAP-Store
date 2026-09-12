import type { ReactNode } from "react";

export type Checked = boolean | "mixed";

type Base = {
  checked: Checked;
  /** Called with what the box becomes: a mixed box becomes checked. */
  onChange: (checked: boolean) => void;
  /** The visible label; when omitted, `name` is the accessible name. */
  children?: ReactNode;
  name?: string;
  /** Inside a row: an 18 px hit area at a fixed x so the column lines up (§7.12). */
  hit?: boolean;
  className?: string;
};

export type CheckboxProps =
  | (Base & { disabled?: false; disabledReason?: undefined })
  | (Base & { disabled: true; disabledReason: string });

/** The painted box alone, for a row that handles its own click. */
export function CheckMark({ checked }: { checked: Checked }) {
  const cls = checked === "mixed" ? "bs-cb mixed" : checked ? "bs-cb on" : "bs-cb";
  return <span className={cls} aria-hidden="true" />;
}

/** 16 × 16, radius 3, accent when on, a dash when mixed. */
export function Checkbox({ checked, onChange, children, name, hit, className, disabled, disabledReason }: CheckboxProps) {
  const mark = <CheckMark checked={checked} />;
  return (
    <button
      type="button"
      role="checkbox"
      aria-checked={checked === "mixed" ? "mixed" : checked}
      aria-label={children ? undefined : name}
      className={["bs-check", className ?? ""].filter(Boolean).join(" ")}
      title={disabled ? disabledReason : undefined}
      disabled={disabled}
      onClick={(e) => {
        e.stopPropagation();
        onChange(checked !== true);
      }}
    >
      {hit ? <span className="bs-cb-hit">{mark}</span> : mark}
      {children}
    </button>
  );
}
