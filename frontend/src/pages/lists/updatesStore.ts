// What the Updates page knows for the session: the last list and when it was
// made, the self-update check, which rows are ticked, and the filters. The
// shell's badge count is set from here whenever a list lands, so the sidebar
// and the page never disagree.

import { create } from "zustand";
import * as api from "../../api";
import { useShell } from "../../shell/store";
import type { SelfUpdate, SourceKind, UpdateList } from "../../types";
import { message, refKey } from "./shared";

export type UpdatesSort = "name" | "size" | "source";

interface UpdatesState {
  list: UpdateList | null;
  loading: boolean;
  error: string | null;
  self: SelfUpdate | null;
  selfError: string | null;
  /** Keys of the ticked rows (source:id). */
  ticked: string[];
  query: string;
  /** null: every available source. */
  sources: SourceKind[] | null;
  sort: UpdatesSort;
  load: (force: boolean) => Promise<void>;
  checkSelf: (force: boolean) => Promise<void>;
  toggle: (key: string) => void;
  tick: (keys: string[], on: boolean) => void;
  setQuery: (query: string) => void;
  setSources: (sources: SourceKind[]) => void;
  setSort: (sort: UpdatesSort) => void;
}

let asked = 0;

/** Only a browser has a query string; the window never does. Screenshots use ?tick=source:id,source:id to start with rows ticked. */
function initialTicked(): string[] {
  if (api.inTauri) return [];
  const tick = new URLSearchParams(window.location.search).get("tick");
  return tick ? tick.split(",").filter((k) => k.includes(":")) : [];
}

export const useUpdates = create<UpdatesState>((set, get) => ({
  list: null,
  loading: false,
  error: null,
  self: null,
  selfError: null,
  ticked: initialTicked(),
  query: "",
  sources: null,
  sort: "name",

  load: async (force) => {
    asked += 1;
    const mine = asked;
    set({ loading: true });
    try {
      const list = await api.updates(force);
      if (mine !== asked) return;
      // A tick on a row that is no longer in the list is dropped with the row.
      const present = new Set(list.updates.map((u) => refKey(u.package)));
      set({ list, error: null, ticked: get().ticked.filter((k) => present.has(k)) });
      useShell.getState().setUpdateCount(list.updates.length);
    } catch (e) {
      if (mine !== asked) return;
      set({ error: message(e) });
    } finally {
      if (mine === asked) set({ loading: false });
    }
  },

  checkSelf: async (force) => {
    try {
      set({ self: await api.self_update_check(force), selfError: null });
    } catch (e) {
      set({ selfError: message(e) });
    }
  },

  toggle: (key) => {
    const { ticked } = get();
    set({ ticked: ticked.includes(key) ? ticked.filter((k) => k !== key) : [...ticked, key] });
  },
  tick: (keys, on) => {
    const { ticked } = get();
    set({ ticked: on ? [...new Set([...ticked, ...keys])] : ticked.filter((k) => !keys.includes(k)) });
  },
  setQuery: (query) => set({ query }),
  setSources: (sources) => set({ sources }),
  setSort: (sort) => set({ sort }),
}));
