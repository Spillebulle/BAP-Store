import { useEffect, useState } from "react";
import { pictureSrc } from "../api";
import type { Picture } from "../types";

export type AppIconSize = "row" | "card" | "hero";

interface Props {
  picture: Picture | null | undefined;
  /** The application's name: the alt text, and the first letter when the picture is missing. */
  name: string;
  size: AppIconSize;
}

/**
 * An application icon at one rung of its own ladder (32 / 48 / 96). A missing
 * or broken picture is a control block with the initial in dim at the same
 * size, never a gap, so a list does not shift as pictures arrive.
 */
export function AppIcon({ picture, name, size }: Props) {
  const src = pictureSrc(picture);
  const [failed, setFailed] = useState(false);
  useEffect(() => setFailed(false), [src]);
  const initial = (name.trim().charAt(0) || "?").toUpperCase();
  const cls = `bs-appicon bs-appicon--${size}`;
  if (!src || failed) {
    return (
      <span className={cls} role="img" aria-label={name} title={name}>
        {initial}
      </span>
    );
  }
  return (
    <span className={cls}>
      <img src={src} alt={name} loading="lazy" decoding="async" onError={() => setFailed(true)} />
    </span>
  );
}
