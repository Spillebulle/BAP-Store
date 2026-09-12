import { HardDrive } from "lucide-react";
import { EmptyState } from "../components/EmptyState";
import { ICON_EMPTY } from "../components/icons";

/** Placeholder. The next phase puts the toolbar and the list of installed applications here. */
export function InstalledPage() {
  return (
    <EmptyState fill icon={<HardDrive {...ICON_EMPTY} aria-hidden="true" />}>
      Installed is not built yet. Everything on this machine will be listed here.
    </EmptyState>
  );
}
