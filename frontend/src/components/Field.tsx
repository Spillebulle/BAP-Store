import type { KeyboardEvent, ReactNode, Ref } from "react";

export interface FieldProps {
  /** The accessible name. */
  label: string;
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
  type?: "text" | "number" | "password" | "url";
  /** A sentence beneath in caution; the border follows. Never a red outline alone (§7.11). */
  error?: string;
  leading?: ReactNode;
  trailing?: ReactNode;
  /** Fill the line. */
  full?: boolean;
  /** A fixed width in CSS units when it must match a control beside it. */
  width?: string;
  autoFocus?: boolean;
  disabled?: boolean;
  disabledReason?: string;
  onKeyDown?: (e: KeyboardEvent<HTMLInputElement>) => void;
  onFocus?: () => void;
  onBlur?: () => void;
  inputRef?: Ref<HTMLInputElement>;
  spellCheck?: boolean;
}

/** A text field (§7.11): field fill, hairline, radius 6, 26 tall; focus is the accent ring on the well. */
export function Field({
  label,
  value,
  onChange,
  placeholder,
  type = "text",
  error,
  leading,
  trailing,
  full,
  width,
  autoFocus,
  disabled,
  disabledReason,
  onKeyDown,
  onFocus,
  onBlur,
  inputRef,
  spellCheck = false,
}: FieldProps) {
  const classes = ["bs-field", full ? "bs-field--full" : "", error ? "bs-field--error" : ""]
    .filter(Boolean)
    .join(" ");
  return (
    <div className={full ? "bs-field-wrap bs-field--full" : "bs-field-wrap"} style={width ? { width } : undefined}>
      <div className={classes} title={disabled ? disabledReason : undefined}>
        {leading}
        <input
          ref={inputRef}
          type={type}
          value={value}
          placeholder={placeholder}
          aria-label={label}
          aria-invalid={error ? true : undefined}
          autoFocus={autoFocus}
          disabled={disabled}
          spellCheck={spellCheck}
          onChange={(e) => onChange(e.target.value)}
          onKeyDown={onKeyDown}
          onFocus={onFocus}
          onBlur={onBlur}
        />
        {trailing}
      </div>
      {error ? <div className="bs-field-error">{error}</div> : null}
    </div>
  );
}
