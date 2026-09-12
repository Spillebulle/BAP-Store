import { Check } from "lucide-react";
import { sourceLabel, type SourceKind } from "../types";
import { Badge } from "./Badge";
import { ICON_MARK } from "./icons";

interface Props {
  source: SourceKind;
  /** This edition is on the machine: a small check joins the word. */
  installed?: boolean;
  /** "extra", "flathub": shown in the tooltip, never in the badge. */
  repo?: string | null;
}

/** Sources are told apart by a neutral badge. Colour means state, never which source. */
export function SourceBadge({ source, installed, repo }: Props) {
  const label = sourceLabel(source);
  const where = repo ? `${label} (${repo})` : label;
  return (
    <Badge title={installed ? `Installed from ${where}.` : `Available from ${where}.`} icon={installed ? <Check {...ICON_MARK} aria-hidden="true" /> : undefined}>
      {label}
    </Badge>
  );
}
