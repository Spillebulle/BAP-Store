import { Cpu } from "lucide-react";
import { EmptyState } from "../components/EmptyState";
import { ICON_EMPTY } from "../components/icons";

/** Placeholder. The next phase puts a panel per device and the firmware panel here. */
export function DriversPage() {
  return (
    <EmptyState fill icon={<Cpu {...ICON_EMPTY} aria-hidden="true" />}>
      Drivers is not built yet. Devices, their profiles and firmware will be listed here.
    </EmptyState>
  );
}
