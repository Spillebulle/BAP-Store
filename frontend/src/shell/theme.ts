import type { Theme } from "../types";

/**
 * One theme, three states (§3.1): "dark" or "light" is stamped as a class on
 * <html>; "system" stamps nothing so prefers-color-scheme decides. tokens.css
 * does the rest, and no component ever reads the class.
 */
export function applyTheme(theme: Theme): void {
  const root = document.documentElement;
  root.classList.remove("dark", "light");
  if (theme !== "system") root.classList.add(theme);
}
