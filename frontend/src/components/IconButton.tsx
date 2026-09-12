import type { ButtonHTMLAttributes, ReactNode } from "react";

type Base = Omit<ButtonHTMLAttributes<HTMLButtonElement>, "disabled" | "title" | "aria-label"> & {
  /** The tooltip and the accessible name. An icon without a label is not allowed (§11). */
  label: string;
  icon: ReactNode;
  /** 26 px in a toolbar or row, 20 px in a panel header. */
  size?: "md" | "sm";
  /** A tool button is 32 px, radius 6, and takes the accent when active. */
  kind?: "icon" | "tool";
  active?: boolean;
};

export type IconButtonProps =
  | (Base & { disabled?: false; disabledReason?: undefined })
  | (Base & { disabled: true; disabledReason: string });

export function IconButton(props: IconButtonProps) {
  const { label, icon, size = "md", kind = "icon", active, className, disabled, disabledReason, type = "button", ...rest } = props;
  const classes = [
    kind === "tool" ? "bs-tool" : "bs-ibtn",
    kind === "icon" && size === "sm" ? "bs-ibtn--sm" : "",
    active ? "on" : "",
    className ?? "",
  ]
    .filter(Boolean)
    .join(" ");
  return (
    <button
      type={type}
      className={classes}
      aria-label={label}
      aria-pressed={kind === "tool" ? Boolean(active) : undefined}
      title={disabled ? disabledReason : label}
      disabled={disabled}
      {...rest}
    >
      {icon}
    </button>
  );
}
