import { ArrowLeft, Package } from "lucide-react";
import { Button } from "../components/Button";
import { EmptyState } from "../components/EmptyState";
import { ICON, ICON_EMPTY } from "../components/icons";
import { useShell } from "../shell/store";

/** Placeholder. The next phase puts the backdrop, the hero, the facts and the media rail here. */
export function AppDetailPage({ appKey }: { appKey?: string }) {
  const back = useShell((s) => s.back);
  return (
    <EmptyState
      fill
      icon={<Package {...ICON_EMPTY} aria-hidden="true" />}
      action={
        <Button icon={<ArrowLeft {...ICON} aria-hidden="true" />} onClick={back}>
          Back
        </Button>
      }
    >
      {appKey ? `The page for ${appKey} is not built yet.` : "Application detail is not built yet."}
    </EmptyState>
  );
}
