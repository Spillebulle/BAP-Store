import { Search } from "lucide-react";
import { EmptyState } from "../components/EmptyState";
import { ICON_EMPTY } from "../components/icons";

/** Placeholder. The next phase puts the toolbar, the result list and the empty state here. */
export function SearchPage() {
  return (
    <EmptyState fill icon={<Search {...ICON_EMPTY} aria-hidden="true" />}>
      Search is not built yet. Results from pacman, the AUR and Flatpak will be listed here.
    </EmptyState>
  );
}
