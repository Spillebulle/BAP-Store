import { RefreshCw } from "lucide-react";
import { EmptyState } from "../components/EmptyState";
import { ICON_EMPTY } from "../components/icons";

/** Placeholder. The next phase puts the update list, the tick boxes and Update all here. */
export function UpdatesPage() {
  return (
    <EmptyState fill icon={<RefreshCw {...ICON_EMPTY} aria-hidden="true" />}>
      Updates is not built yet. Updates from every source will be listed here.
    </EmptyState>
  );
}
