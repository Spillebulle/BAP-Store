import { Image } from "lucide-react";
import { useEffect, useState, type ReactNode } from "react";
import { pictureSrc } from "../api";
import type { Picture } from "../types";
import { ICON_EMPTY } from "./icons";

export type ArtSize = "row" | "tile" | "card" | "hero";

interface ArtProps {
  picture: Picture | null | undefined;
  /** Names what the picture is: "Blender's node editor", never "screenshot". */
  alt: string;
  size: ArtSize;
  /** Screenshots are landscape 16/9; posters are portrait 2/3. */
  landscape?: boolean;
  /** A small state mark in the corner on a flat scrim. */
  state?: ReactNode;
  className?: string;
}

/** A picture at one rung of the ladder (§7.21), holding its box while it loads or when it is missing. */
export function Art({ picture, alt, size, landscape = true, state, className }: ArtProps) {
  const src = pictureSrc(picture);
  const [failed, setFailed] = useState(false);
  useEffect(() => setFailed(false), [src]);
  const cls = ["bs-art", `bs-art--${size}`, landscape ? "bs-art--landscape" : "bs-art--portrait", className ?? ""]
    .filter(Boolean)
    .join(" ");
  return (
    <span className={cls}>
      {src && !failed ? (
        <img src={src} alt={alt} loading="lazy" decoding="async" onError={() => setFailed(true)} />
      ) : (
        <span className="bs-art-fallback" role="img" aria-label={alt}>
          <Image {...ICON_EMPTY} aria-hidden="true" />
        </span>
      )}
      {state ? <span className="bs-art-state">{state}</span> : null}
    </span>
  );
}

interface ArtCardProps extends Omit<ArtProps, "className"> {
  title: string;
  /** One figure under the title: a year, a size, a count. Two lines, never three. */
  figure?: string;
  selected?: boolean;
  onClick?: () => void;
}

/**
 * The art card: the picture is the item. Resting, it is the artwork alone;
 * under the pointer or keyboard focus it lifts and the label fades in on a
 * black scrim. Where hover does not exist the label is always there.
 */
export function ArtCard({ picture, alt, size, landscape, state, title, figure, selected, onClick }: ArtCardProps) {
  const src = pictureSrc(picture);
  const [failed, setFailed] = useState(false);
  useEffect(() => setFailed(false), [src]);
  const cls = ["bs-art", "bs-artcard", `bs-art--${size}`, landscape === false ? "bs-art--portrait" : "bs-art--landscape", selected ? "on" : ""]
    .filter(Boolean)
    .join(" ");
  return (
    <button type="button" className={cls} aria-label={title} aria-pressed={selected} onClick={onClick}>
      {src && !failed ? (
        <img src={src} alt={alt} loading="lazy" decoding="async" onError={() => setFailed(true)} />
      ) : (
        <span className="bs-art-fallback" aria-hidden="true">
          <Image {...ICON_EMPTY} />
        </span>
      )}
      {state ? <span className="bs-art-state">{state}</span> : null}
      <span className="bs-artcard-label">
        <b>{title}</b>
        {figure ? <span>{figure}</span> : null}
      </span>
    </button>
  );
}
