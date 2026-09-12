import { lazy, Suspense, useEffect, useRef } from "react";
import { ActivityPanel } from "./activity/ActivityPanel";
import { subscribeActivity } from "./activity/store";
import * as api from "./api";
import { Toasts } from "./components/Toast";
import { toast } from "./components/toastStore";
import { AppDetailPage, DriversPage, InstalledPage, SearchPage, SettingsPage, UpdatesPage } from "./pages";
import { Shell } from "./shell/Shell";
import { useShell } from "./shell/store";
import { applyTheme } from "./shell/theme";

// The component sheet is a development aid for a browser only; the window
// never loads it, so it is split out of the bundle it would otherwise weigh on.
const Gallery = lazy(() => import("./dev/Gallery").then((m) => ({ default: m.Gallery })));

function Page() {
  const view = useShell((s) => s.view);
  const appKey = useShell((s) => s.appKey);
  switch (view) {
    case "search":
      return <SearchPage />;
    case "app":
      return <AppDetailPage appKey={appKey} />;
    case "installed":
      return <InstalledPage />;
    case "updates":
      return <UpdatesPage />;
    case "drivers":
      return <DriversPage />;
    case "settings":
      return <SettingsPage />;
  }
}

async function checkUpdatesOnStart() {
  const { setStatus, setUpdateCount } = useShell.getState();
  setStatus("Checking for updates…");
  try {
    const list = await api.updates(false);
    setUpdateCount(list.updates.length);
    for (const [, sentence] of list.failed) toast(sentence, "caution");
  } catch (e) {
    toast(e instanceof Error ? e.message : String(e), "error");
  } finally {
    setStatus(null);
  }
}

export default function App() {
  const load = useShell((s) => s.load);
  const theme = useShell((s) => s.settings?.theme);
  const checkOnStart = useShell((s) => s.settings?.check_updates_on_start);
  const checked = useRef(false);

  useEffect(() => {
    const unsubscribe = subscribeActivity();
    void load();
    return unsubscribe;
  }, [load]);

  useEffect(() => {
    applyTheme(theme ?? "system");
  }, [theme]);

  // Once, after settings have loaded and only if they ask for it.
  useEffect(() => {
    if (checkOnStart !== true || checked.current) return;
    checked.current = true;
    void checkUpdatesOnStart();
  }, [checkOnStart]);

  const gallery = !api.inTauri && new URLSearchParams(window.location.search).has("gallery");

  return (
    <>
      <Shell>{gallery ? <Suspense fallback={null}><Gallery /></Suspense> : <Page />}</Shell>
      <div className="bs-corner">
        <Toasts />
        <ActivityPanel />
      </div>
    </>
  );
}
