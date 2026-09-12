import type { ButtonHTMLAttributes, ReactNode } from "react";

export type ButtonKind = "primary" | "secondary" | "outline" | "ghost" | "danger";

type Base = Omit<ButtonHTMLAttributes<HTMLButtonElement>, "disabled" | "title"> & {
  /** Primary is the one thing to do on the view; at most one visible. */
  kind?: ButtonKind;
  icon?: ReactNode;
  title?: string;
  children?: ReactNode;
};

// A disabled control explains itself (§12): the type makes the reason
// compulsory rather than trusting every call site to remember.
export type ButtonProps =
  | (Base & { disabled?: false; disabledReason?: undefined })
  | (Base & { disabled: true; disabledReason: string });

export function Button(props: ButtonProps) {
  const { kind = "secondary", icon, children, className, disabled, disabledReason, title, type = "button", ...rest } = props;
  const classes = ["bs-btn", kind !== "secondary" ? `bs-btn--${kind}` : "", className ?? ""]
    .filter(Boolean)
    .join(" ");
  return (
    <button
      type={type}
      className={classes}
      disabled={disabled}
      title={disabled ? disabledReason : title}
      {...rest}
    >
      {icon}
      {children}
    </button>
  );
}
