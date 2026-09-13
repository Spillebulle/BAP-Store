import { Search, X } from "lucide-react";
import type { KeyboardEvent, Ref } from "react";
import { Field } from "./Field";
import { ICON } from "./icons";

interface Props {
  label: string;
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
  /** Enter. The page decides whether typing already searches. */
  onSubmit?: (value: string) => void;
  full?: boolean;
  width?: string;
  autoFocus?: boolean;
  inputRef?: Ref<HTMLInputElement>;
}

/** The search well: a magnifier leading, a clear mark trailing while there is text. Escape clears. */
export function SearchField({ label, value, onChange, placeholder, onSubmit, full, width, autoFocus, inputRef }: Props) {
  const onKeyDown = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Enter") {
      onSubmit?.(value);
    } else if (e.key === "Escape" && value) {
      e.stopPropagation();
      onChange("");
    }
  };
  return (
    <Field
      label={label}
      value={value}
      onChange={onChange}
      placeholder={placeholder}
      full={full}
      width={width}
      autoFocus={autoFocus}
      inputRef={inputRef}
      onKeyDown={onKeyDown}
      leading={<Search {...ICON} aria-hidden="true" />}
      trailing={
        value ? (
          <button type="button" className="bk-field-clear" title="Clear" aria-label="Clear" onClick={() => onChange("")}>
            <X {...ICON} size={14} aria-hidden="true" />
          </button>
        ) : null
      }
    />
  );
}
