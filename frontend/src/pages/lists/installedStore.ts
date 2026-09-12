// What the Installed page knows, kept for the session so coming back to the
// page shows the list at once while the sources are asked again behind it.
// The filters live here too, so a trip to a detail page and back finds them
// as they were.

import { create } from "zustand";
import * as api from "../../api";
import type { SearchResult, SourceKind } from "../../types";
import { message } from "./shared";

export type Scope = "apps" | "all";
export type InstalledSort = "name" | "source" | "size" | "updated";

interface InstalledState {
  result: SearchResult | null;
  loading: boolean;
  /** What the last ask failed with; the previous list stays on screen beside it. */
  error: string | null;
  query: string;
  /** null: every available source. */
  sources: SourceKind[] | null;
  /** null: follow settings.show_packages. */
  scope: Scope | null;
  sort: InstalledSort;
  load: () => Promise<void>;
  setQuery: (query: string) => void;
  setSources: (sources: SourceKind[]) => void;
  setScope: (scope: Scope) => void;
  setSort: (sort: InstalledSort) => void;
}

let asked = 0;

export const useInstalled = create<InstalledState>((set) => ({
  result: null,
  loading: false,
  error: null,
  query: "",
  sources: null,
  scope: null,
  sort: "name",

  load: async () => {
    asked += 1;
    const mine = asked;
    set({ loading: true });
    try {
      const result = await api.installed();
      // A later ask answers for the page; an earlier one arriving late does not.
      if (mine !== asked) return;
      set({ result, error: null });
    } catch (e) {
      if (mine !== asked) return;
      set({ error: message(e) });
    } finally {
      if (mine === asked) set({ loading: false });
    }
  },

  setQuery: (query) => set({ query }),
  setSources: (sources) => set({ sources }),
  setScope: (scope) => set({ scope }),
  setSort: (sort) => set({ sort }),
}));
