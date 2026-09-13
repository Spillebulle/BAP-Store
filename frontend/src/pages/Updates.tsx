// Updates: every source's updates in one table with tick boxes, Brokey's
// own update first, honest sizes, and the Arch partial-upgrade notice drawn
// the moment a subset of pacman rows is ticked. Update all is the primary
// action; the activity panel takes over once a plan starts.

import { ArrowRight, ArrowUpDown, CircleCheck, ExternalLink, Layers, RefreshCw } from "lucide-react";
import { useEffect, useMemo, useState, type ReactNode } from "react";
import { startOps } from "../activity/flow";
import { useActivity } from "../activity/store";
import * as api from "../api";
import { AppIcon, Button, Bytes, Checkbox, Count, Dropdown, EmptyState, Figure, ICON, ICON_EMPTY, MultiSelect, Notice, SearchField, Skeleton, SourceBadge, When, toast } from "../components";
import type { DropdownOption } from "../components";
import { useShell } from "../shell/store";
import { SOURCE_KINDS, type Op, type SelfUpdate, type SourceKind, type Update, type UpdateList } from "../types";
import { availableKinds, failedSentence, isNewer, message, off, onPlanDone, refKey, sourceOptions } from "./lists/shared";
import { useUpdates, type UpdatesSort } from "./lists/updatesStore";
import "./lists/lists.css";

const SORTS: DropdownOption<UpdatesSort>[] = [
  { value: "name", label: "Name" },
  { value: "size", label: "Size" },
  { value: "source", label: "Source" },
];

const PARTIAL_UPGRADE = "Arch does not support partial upgrades, so updating any pacman package updates every pacman package. Update all is what runs.";

/** An unknown figure is an en dash in dim, never a zero (§7.14). */
function Unknown({ title }: { title: string }) {
  return (
    <span className="bl-unknown" title={title}>
      –
    </span>
  );
}

function compare(sort: UpdatesSort): (a: Update, b: Update) => number {
  const byName = (a: Update, b: Update) => a.name.localeCompare(b.name);
  switch (sort) {
    case "name":
      return byName;
    case "size":
      return (a, b) => (b.download_size ?? -1) - (a.download_size ?? -1) || byName(a, b);
    case "source":
      return (a, b) => SOURCE_KINDS.indexOf(a.package.source) - SOURCE_KINDS.indexOf(b.package.source) || byName(a, b);
  }
}

function matches(u: Update, query: string): boolean {
  const q = query.trim().toLowerCase();
  if (!q) return true;
  return u.name.toLowerCase().includes(q) || (u.summary ?? "").toLowerCase().includes(q) || u.package.id.toLowerCase().includes(q);
}

/** The bytes to download for these rows: the known total, and whether every row had a size. */
function download(rows: Update[]): { total: number; complete: boolean; any: boolean } {
  let total = 0;
  let known = 0;
  for (const u of rows) {
    if (u.download_size !== null) {
      total += u.download_size;
      known += 1;
    }
  }
  return { total, complete: known === rows.length, any: known > 0 };
}

function DownloadSize({ rows }: { rows: Update[] }) {
  const { total, complete, any } = download(rows);
  if (!any) return <>download size not known</>;
  return (
    <>
      {complete ? "" : "at least "}
      <Bytes value={total} /> to download
    </>
  );
}

/** "3 updates · 35 kB to download · checked 40 min ago". */
function HeadLine({ list, loading }: { list: UpdateList | null; loading: boolean }) {
  if (!list) return loading ? <Skeleton width="40%" /> : null;
  if (loading) return <>Checking for updates…</>;
  const n = list.updates.length;
  if (n === 0) {
    return (
      <>
        No updates · checked <When value={list.checked_at} />
      </>
    );
  }
  return (
    <>
      <Count value={n} /> {n === 1 ? "update" : "updates"} · <DownloadSize rows={list.updates} /> · checked <When value={list.checked_at} />
    </>
  );
}

/** The notice for a newer Brokey, saying the one true thing about how to get it (§18.3). */
function SelfUpdateNotice({ self, hasSelfRow }: { self: SelfUpdate; hasSelfRow: boolean }) {
  const [busy, setBusy] = useState(false);
  const latest = self.latest;
  if (!latest || !isNewer(latest.version, self.current)) return null;
  const remedy = self.remedy;

  let sentence: string;
  if (!remedy) sentence = `${self.installation.label}.`;
  else if (remedy.kind === "updates_page") sentence = hasSelfRow ? `It is in this list as ${remedy.package}. Tick it and it updates with the rest.` : remedy.sentence;
  else sentence = remedy.sentence;

  const canApply = remedy?.kind === "install_asset" || remedy?.kind === "replace_file";
  const apply = async () => {
    setBusy(true);
    try {
      const status = await api.self_update_apply();
      useActivity.getState().track(status);
    } catch (e) {
      toast(message(e), "error");
    } finally {
      setBusy(false);
    }
  };

  return (
    <Notice
      actions={
        <>
          {canApply ? (
            <Button kind="ghost" onClick={() => void apply()} {...off(busy, "The update is starting.")}>
              Update Brokey
            </Button>
          ) : null}
          <Button kind="ghost" icon={<ExternalLink {...ICON} aria-hidden="true" />} title="Open the release on GitHub." onClick={() => void api.openUrl(latest.url)}>
            Release notes
          </Button>
        </>
      }
    >
      Brokey <Figure className="bl-figure">{latest.version}</Figure> is available. {sentence}
    </Notice>
  );
}

function Header({ checked, onChange, busy }: { checked: boolean | "mixed"; onChange: (on: boolean) => void; busy?: boolean }) {
  return (
    <thead>
      <tr>
        <th className="bl-c-narrow">{busy ? <Skeleton className="bl-skel-box" /> : <Checkbox hit name="Tick every update shown" checked={checked} onChange={onChange} />}</th>
        <th className="bl-c-narrow" />
        <th className="bl-c-name">Name</th>
        <th className="bl-c-narrow">Source</th>
        <th className="bl-c-narrow">From</th>
        <th className="bl-c-narrow" />
        <th className="bl-c-narrow">To</th>
        <th className="bl-c-narrow bl-c-size">Size</th>
        <th className="bl-c-narrow" />
      </tr>
    </thead>
  );
}

function SkeletonRows({ count }: { count: number }) {
  return (
    <table className="bk-table bl-table" aria-busy="true" aria-label="Loading">
      <Header busy checked={false} onChange={() => {}} />
      <tbody>
        {Array.from({ length: count }, (_, i) => (
          <tr key={i} aria-busy="true">
            <td className="bl-c-narrow">
              <Skeleton className="bl-skel-box" />
            </td>
            <td className="bl-c-narrow">
              <Skeleton className="bk-skel--icon" />
            </td>
            <td className="bl-c-name">
              <div className="bl-skel-lines">
                <Skeleton width={`${24 + ((i * 17) % 30)}%`} />
                <Skeleton width={`${40 + ((i * 23) % 40)}%`} className="bk-skel--text" />
              </div>
            </td>
            <td className="bl-c-narrow">
              <Skeleton className="bl-skel-badge" />
            </td>
            <td className="bl-c-narrow">
              <Skeleton className="bl-skel-figure" />
            </td>
            <td className="bl-c-narrow" />
            <td className="bl-c-narrow">
              <Skeleton className="bl-skel-figure" />
            </td>
            <td className="bl-c-narrow bl-c-size">
              <Skeleton className="bl-skel-figure" />
            </td>
            <td className="bl-c-narrow" />
          </tr>
        ))}
      </tbody>
    </table>
  );
}

function UpdateRow({ update, ticked, onToggle }: { update: Update; ticked: boolean; onToggle: () => void }) {
  const u = update;
  const single = () => void startOps([{ op: "update", package: u.package }], `Update ${u.name}`);
  const alone = u.package.source === "pacman" ? `Update ${u.name}. On Arch this runs the full system upgrade with ${u.name} included.` : `Update ${u.name} alone.`;
  return (
    <tr onClick={onToggle}>
      <td className="bl-c-narrow">
        <Checkbox hit name={`Tick ${u.name}`} checked={ticked} onChange={onToggle} />
      </td>
      <td className="bl-c-narrow">
        <AppIcon picture={u.icon} name={u.name} size="row" />
      </td>
      <td className="bl-c-name">
        <div className="bl-name">
          <span className="bl-name-label">{u.name}</span>
          {u.is_self ? <span className="bk-dot bk-dot--accent" title="This is Brokey itself." /> : null}
        </div>
        <div className="bk-row-sub">{u.summary ?? u.package.id}</div>
      </td>
      <td className="bl-c-narrow">
        <SourceBadge source={u.package.source} installed />
      </td>
      <td className="bl-c-narrow">{u.from ? <Figure className="bl-figure">{u.from}</Figure> : <Unknown title="The installed version is not known." />}</td>
      <td className="bl-c-narrow bl-c-arrow">
        <ArrowRight {...ICON} aria-label="to" />
      </td>
      <td className="bl-c-narrow">
        <Figure className="bl-figure">{u.to}</Figure>
      </td>
      <td className="bl-c-narrow bl-c-size">{u.download_size !== null ? <Bytes value={u.download_size} /> : <Unknown title="The download size is not known." />}</td>
      <td className="bl-c-narrow" onClick={(e) => e.stopPropagation()} onKeyDown={(e) => e.stopPropagation()}>
        <Button kind="ghost" title={alone} onClick={single}>
          Update
        </Button>
      </td>
    </tr>
  );
}

export function UpdatesPage() {
  const shellSources = useShell((s) => s.sources);
  const list = useUpdates((s) => s.list);
  const loading = useUpdates((s) => s.loading);
  const error = useUpdates((s) => s.error);
  const self = useUpdates((s) => s.self);
  const ticked = useUpdates((s) => s.ticked);
  const query = useUpdates((s) => s.query);
  const sources = useUpdates((s) => s.sources);
  const sort = useUpdates((s) => s.sort);
  const { load, checkSelf, toggle, tick, setQuery, setSources, setSort } = useUpdates.getState();

  useEffect(() => {
    void load(false);
    void checkSelf(false);
    // A finished transaction changes what is out of date; ask again rather than show the old list.
    return onPlanDone(() => void load(false));
  }, [load, checkSelf]);

  const checkAgain = () => {
    void load(true);
    void checkSelf(true);
  };

  const options = useMemo(() => sourceOptions(shellSources), [shellSources]);
  const chosen = useMemo(() => sources ?? availableKinds(shellSources), [sources, shellSources]);
  const updates = list?.updates ?? null;

  const shown = useMemo(() => {
    if (!updates) return [];
    const wanted = new Set<SourceKind>(chosen);
    const cmp = compare(sort);
    return updates
      .filter((u) => wanted.has(u.package.source))
      .filter((u) => matches(u, query))
      .sort((a, b) => Number(b.is_self) - Number(a.is_self) || cmp(a, b));
  }, [updates, chosen, query, sort]);

  const tickedSet = useMemo(() => new Set(ticked), [ticked]);
  const tickedRows = useMemo(() => (updates ?? []).filter((u) => tickedSet.has(refKey(u.package))), [updates, tickedSet]);
  const shownKeys = shown.map((u) => refKey(u.package));
  const shownTicked = shownKeys.filter((k) => tickedSet.has(k)).length;
  const headerState: boolean | "mixed" = shown.length > 0 && shownTicked === shown.length ? true : shownTicked > 0 ? "mixed" : false;

  // Ticking some pacman rows but not all of them is a partial upgrade on Arch.
  const pacmanRows = (updates ?? []).filter((u) => u.package.source === "pacman");
  const pacmanTicked = tickedRows.filter((u) => u.package.source === "pacman");
  const partial = pacmanTicked.length > 0 && pacmanTicked.length < pacmanRows.length;

  const updateAll = () => {
    if (!updates) return;
    const kinds = SOURCE_KINDS.filter((k) => updates.some((u) => u.package.source === k));
    const ops: Op[] = kinds.map((source) => ({ op: "updateall", source }));
    void startOps(ops, kinds.includes("fwupd") ? "Update all, including firmware" : "Update all");
  };
  const updateSelected = () => {
    if (tickedRows.length === 0) return;
    const ops: Op[] = tickedRows.map((u) => ({ op: "update", package: u.package }));
    void startOps(ops, tickedRows.length === 1 ? `Update ${tickedRows[0].name}` : `Update ${tickedRows.length} packages`);
  };

  const filtered = query.trim() !== "" || sources !== null;
  const clearFilter = () => {
    setQuery("");
    setSources(availableKinds(shellSources));
  };

  const checkButton = (
    <Button kind="outline" icon={<RefreshCw {...ICON} aria-hidden="true" />} title="Ask every source for updates again." onClick={checkAgain} {...off(loading, "A check is running now.")}>
      Check again
    </Button>
  );

  let body: ReactNode;
  if (!updates) {
    body = loading ? (
      <div className="bk-well bl-well">
        <SkeletonRows count={6} />
      </div>
    ) : null;
  } else if (updates.length === 0) {
    body = (
      <EmptyState fill icon={<CircleCheck {...ICON_EMPTY} aria-hidden="true" />} action={checkButton}>
        Everything is up to date. Checked <When value={list?.checked_at} />.
      </EmptyState>
    );
  } else if (shown.length === 0) {
    body = (
      <EmptyState fill icon={<CircleCheck {...ICON_EMPTY} aria-hidden="true" />} action={filtered ? <Button onClick={clearFilter}>Clear filter</Button> : undefined}>
        No update matches this filter.
      </EmptyState>
    );
  } else {
    body = (
      <div className="bk-well bl-well">
        <table className="bk-table bl-table">
          <Header checked={headerState} onChange={(on) => tick(shownKeys, on)} />
          <tbody>
            {shown.map((u) => {
              const key = refKey(u.package);
              return <UpdateRow key={key} update={u} ticked={tickedSet.has(key)} onToggle={() => toggle(key)} />;
            })}
          </tbody>
        </table>
      </div>
    );
  }

  return (
    <div className="bl-page">
      <div className="bk-toolbar bl-toolbar">
        <SearchField label="Filter updates" value={query} onChange={setQuery} placeholder="Filter by name or summary" width="calc(var(--s6) * 8)" />
        <MultiSelect<SourceKind> name="Sources" icon={<Layers {...ICON} aria-hidden="true" />} options={options} values={chosen} onChange={setSources} />
        <Dropdown<UpdatesSort> name="Sort" icon={<ArrowUpDown {...ICON} aria-hidden="true" />} options={SORTS} value={sort} onChange={setSort} />
        <div className="bk-toolbar-end">
          {updates && updates.length > 0 ? (
            <span className="bk-small bk-dim">
              <Count value={tickedRows.length} /> selected
            </span>
          ) : null}
          {checkButton}
        </div>
      </div>
      <div className="bk-page bl-body">
        <div className="bk-page-head">
          <h1 className="bk-page-title">Updates</h1>
          <div className="bk-page-sub">
            <HeadLine list={list} loading={loading} />
          </div>
        </div>
        {self ? <SelfUpdateNotice self={self} hasSelfRow={(updates ?? []).some((u) => u.is_self)} /> : null}
        {error ? (
          <Notice
            actions={
              <Button kind="ghost" onClick={checkAgain}>
                Try again
              </Button>
            }
          >
            {error}
          </Notice>
        ) : null}
        {list?.failed.map(([kind, sentence]) => (
          <Notice key={kind}>{failedSentence(kind, sentence, "checked")}</Notice>
        ))}
        {body}
      </div>
      {updates && updates.length > 0 ? (
        <div className="bl-foot">
          {partial ? <Notice>{PARTIAL_UPGRADE}</Notice> : null}
          <div className="bl-foot-row">
            <span className="bl-foot-note">
              {tickedRows.length > 0 ? (
                <>
                  <Count value={tickedRows.length} /> of <Count value={updates.length} /> selected · <DownloadSize rows={tickedRows} />
                </>
              ) : (
                "Nothing is selected. Update all brings every source up to date in one go."
              )}
            </span>
            <div className="bk-btn-group">
              <Button onClick={updateSelected} {...off(tickedRows.length === 0, "Tick something to update.")}>
                Update selected
              </Button>
              <Button kind="primary" onClick={updateAll}>
                Update all
              </Button>
            </div>
          </div>
        </div>
      ) : null}
    </div>
  );
}
