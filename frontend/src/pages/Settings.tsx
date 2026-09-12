import { Settings } from "lucide-react";
import { EmptyState } from "../components/EmptyState";
import { ICON_EMPTY } from "../components/icons";

/** Placeholder. The next phase puts the theme cards, the sources and the update checks here. */
export function SettingsPage() {
  return (
    <EmptyState fill icon={<Settings {...ICON_EMPTY} aria-hidden="true" />}>
      Settings is not built yet. Theme, sources and update checks will be set here.
    </EmptyState>
  );
}
