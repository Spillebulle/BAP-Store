// The current search, kept outside the page so the detail page can read the
// App it was opened from and Back returns to exactly what was on screen: the
// text, the filters, the results and the selected row. The debounce and the
// stale-answer guard live here too, so a search started while the page was
// showing lands even if the user has already opened a row.

import { create } from "zustand";
import { useActivity } from "../../activity/store";
import * as api from "../../api";
import type { App, Op, Package, SearchResult, Settings, SourceKind, SourceStatus } from "../../types";

export type Kind = "app" | "all";
export type Sort = "relevance" | "name" | "updated" | "popularity" | "size";

const DEBOUNCE_MS = 300;
const MIN_CHARS = 2;
const LIMIT = 200;

interface SearchState {
  text: string;
  sources: SourceKind[];
  kind: Kind;
  sort: Sort;
  installedOnly: boolean;
  /** Sources and kind have been taken from settings once. */
  initialised: boolean;
  results: SearchResult | null;
  /** The text and sources `results` answer, so the page can say what was searched. */
  searched: { text: string; sources: SourceKind[] } | null;
  loading: boolean;
  error: string | null;
  /** Index into the visible list; -1 when nothing is selected. */
  selected: number;
  /** Apps opened from any page, by key, so the detail page can find them after the results have moved on. */
  remembered: Record<string, App>;

  init: (settings: Settings, statuses: SourceStatus[]) => void;
  setText: (text: string) => void;
  setSources: (sources: SourceKind[]) => void;
  setKind: (kind: Kind) => void;
  setSort: (sort: Sort) => void;
  setInstalledOnly: (on: boolean) => void;
  setSelected: (index: number) => void;
  /** Run the current query again now (after a split, say). */
  refresh: () => void;
  /** A plan finished: mark what it installed or removed so the rows tell the truth without a new search. */
  applyOps: (ops: Op[]) => void;
}

function message(e: unknown): string {
  return e instanceof Error ? e.message : String(e);
}

let timer: number | null = null;
let seq = 0;

function tooShort(text: string): boolean {
  return text.trim().length < MIN_CHARS;
}

/** Only a browser has a query string; ?q= seeds the field so a picture of results can be taken by URL. The window never does. */
function initialText(): string {
  if (api.inTauri) return "";
  return new URLSearchParams(window.location.search).get("q") ?? "";
}

export const useSearch = create<SearchState>((set, get) => {
  const run = async () => {
    if (timer !== null) {
      window.clearTimeout(timer);
      timer = null;
    }
    const { text, sources } = get();
    const trimmed = text.trim();
    if (trimmed.length < MIN_CHARS) {
      seq += 1;
      set({ results: null, searched: null, loading: false, error: null, selected: -1 });
      return;
    }
    const mine = ++seq;
    set({ loading: true, error: null });
    try {
      const results = await api.search({ text: trimmed, sources: [...sources], limit: LIMIT });
      if (mine !== seq) return;
      set({ results, searched: { text: trimmed, sources: [...sources] }, loading: false, selected: -1 });
    } catch (e) {
      if (mine !== seq) return;
      set({ loading: false, error: message(e) });
    }
  };

  // Typing waits 300 ms for the next key; clearing the field answers at once.
  // The wait counts as loading, so the page draws skeletons rather than nothing.
  const schedule = () => {
    if (tooShort(get().text)) {
      void run();
      return;
    }
    set({ loading: true });
    if (timer !== null) window.clearTimeout(timer);
    timer = window.setTimeout(() => {
      timer = null;
      void run();
    }, DEBOUNCE_MS);
  };

  return {
    text: initialText(),
    sources: [],
    kind: "app",
    sort: "relevance",
    installedOnly: false,
    initialised: false,
    results: null,
    searched: null,
    loading: false,
    error: null,
    selected: -1,
    remembered: {},

    init: (settings, statuses) => {
      if (get().initialised) return;
      // A source searches when its tool is here, or when its public store answers without it.
      const sources = settings.enabled_sources.filter((k) => statuses.some((s) => s.kind === k && (s.available || s.searchable)));
      set({ sources, kind: settings.show_packages ? "all" : "app", initialised: true });
      // Text that was seeded (?q=) was not typed: nothing to wait for.
      if (!tooShort(get().text)) void run();
    },
    setText: (text) => {
      set({ text });
      schedule();
    },
    setSources: (sources) => {
      set({ sources, selected: -1 });
      schedule();
    },
    setKind: (kind) => set({ kind, selected: -1 }),
    setSort: (sort) => set({ sort, selected: -1 }),
    setInstalledOnly: (installedOnly) => set({ installedOnly, selected: -1 }),
    setSelected: (selected) => set({ selected }),
    refresh: () => {
      void run();
    },

    applyOps: (ops) => {
      const { results, remembered } = get();
      const patch = (app: App) => patchApp(app, ops);
      set({
        results: results ? { ...results, apps: results.apps.map(patch) } : null,
        remembered: Object.fromEntries(Object.entries(remembered).map(([k, a]) => [k, patch(a)])),
      });
    },
  };
});

function patchPackage(p: Package, ops: Op[]): Package {
  let next = p;
  for (const op of ops) {
    if (op.op === "updateall") {
      if (op.source === p.source && p.installed) next = { ...next, installed_version: next.version };
      continue;
    }
    if (!("package" in op) || op.package.source !== p.source || op.package.id !== p.id) continue;
    if (op.op === "install") next = { ...next, installed: true, installed_version: next.version };
    else if (op.op === "remove") next = { ...next, installed: false, installed_version: null };
    else if (op.op === "update") next = { ...next, installed_version: next.version };
  }
  return next;
}

function patchApp(app: App, ops: Op[]): App {
  let changed = false;
  const editions = app.editions.map((e) => {
    const pkg = patchPackage(e.package, ops);
    if (pkg === e.package) return e;
    changed = true;
    return { ...e, package: pkg };
  });
  if (!changed) return app;
  return { ...app, editions, installed: editions.some((e) => e.package.installed) };
}

// A plan that finished changes what is installed; the rows follow without a
// new search, and the detail page re-reads its packages on the same signal.
useActivity.subscribe((state, previous) => {
  for (const id of state.order) {
    const now = state.plans[id];
    const before = previous.plans[id];
    if (now && now.state === "done" && before?.state !== "done") useSearch.getState().applyOps(now.plan.ops);
  }
});

/**
 * Keep an App so the detail page can draw it whatever page it was opened from.
 * Call it before `openApp(app.key)`; the Installed and Updates pages do the
 * same with their own rows.
 */
export function rememberApp(app: App): void {
  useSearch.setState((s) => ({ remembered: { ...s.remembered, [app.key]: app } }));
}

/** The App for a key: the current results first (they are fresher), then whatever was remembered. */
export function findApp(state: SearchState, key: string): App | undefined {
  return state.results?.apps.find((a) => a.key === key) ?? state.remembered[key];
}
