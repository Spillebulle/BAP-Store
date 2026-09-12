// The detail page finds an application through the search store's
// rememberApp(app). That store is written beside this one and may not exist
// in a given checkout, so the module is reached through import.meta.glob: a
// glob that matches nothing is an empty record rather than a build error,
// and one that matches is bundled lazily like any other dynamic import.

import type { App } from "../../types";

interface SearchStoreModule {
  rememberApp?: (app: App) => void;
}

const modules = import.meta.glob<SearchStoreModule>("../search/store.ts");

/** Tell the search store about an application about to be opened; silent when there is no store. */
export function rememberApp(app: App): void {
  const load = modules["../search/store.ts"];
  if (!load) return;
  load()
    .then((m) => m.rememberApp?.(app))
    .catch(() => {});
}
