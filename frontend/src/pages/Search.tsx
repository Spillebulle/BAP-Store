import { ArrowDownUp, Check, Download, Search } from "lucide-react";
import { useEffect, useMemo, useRef, type ReactNode } from "react";
import { startOps } from "../activity/flow";
import {
  AppRow,
  Badge,
  Button,
  Dropdown,
  EmptyState,
  Figure,
  ICON,
  ICON_EMPTY,
  ICON_MARK,
  MultiSelect,
  Notice,
  SearchField,
  Segmented,
  SkeletonAppRows,
  Toggle,
  type DropdownOption,
} from "../components";
import { formatCount } from "../format";
import { useShell } from "../shell/store";
import { sourceLabel, type App, type Edition, type SourceKind } from "../types";
import { editionKey, installOptions, joinAnd, primaryEdition, refOf, useBusyRefs } from "./app/editions";
import { rememberApp, useSearch, type Kind, type Sort } from "./search/store";
import "./search/search.css";

const SORTS: DropdownOption<Sort>[] = [
  { value: "relevance", label: "Relevance" },
  { value: "name", label: "Name" },
  { value: "updated", label: "Last updated" },
  { value: "popularity", label: "Popularity" },
  { value: "size", label: "Size" },
];

const KINDS: { value: Kind; label: string }[] = [
  { value: "app", label: "Applications" },
  { value: "all", label: "All packages" },
];

/** Descending with unknowns last, so a missing figure never sorts as zero. */
function desc(a: number | null, b: number | null): number {
  if (a === null && b === null) return 0;
  if (a === null) return 1;
  if (b === null) return -1;
  return b - a;
}

function byName(a: App, b: App): number {
  return a.name.localeCompare(b.name, "en", { sensitivity: "base" });
}

function sizeOf(app: App): number | null {
  const p = primaryEdition(app)?.package;
  return p?.download_size ?? p?.installed_size ?? null;
}

function sortApps(apps: App[], sort: Sort): App[] {
  const out = [...apps];
  switch (sort) {
    case "relevance":
      return out.sort((a, b) => b.relevance - a.relevance || desc(a.popularity, b.popularity) || byName(a, b));
    case "name":
      return out.sort(byName);
    case "updated":
      return out.sort((a, b) => desc(a.updated, b.updated) || byName(a, b));
    case "popularity":
      return out.sort((a, b) => desc(a.popularity, b.popularity) || byName(a, b));
    case "size":
      return out.sort((a, b) => desc(sizeOf(a), sizeOf(b)) || byName(a, b));
  }
}

function install(app: App, edition: Edition) {
  void startOps([{ op: "install", package: refOf(edition) }], `Install ${app.name}`);
}

/** The one action at the right of a row: Install, an edition picker, or the installed mark. */
function RowAction({ app, busy }: { app: App; busy: Map<string, string> }) {
  if (app.installed) {
    return (
      <Badge tone="good" icon={<Check {...ICON_MARK} aria-hidden="true" />} title="An edition of this application is on this machine.">
        Installed
      </Badge>
    );
  }
  if (app.editions.some((e) => busy.get(editionKey(e)) === "install")) {
    return (
      <Button disabled disabledReason="It is being installed. The activity panel shows progress.">
        Installing…
      </Button>
    );
  }
  if (app.editions.length === 1) {
    return (
      <Button icon={<Download {...ICON} aria-hidden="true" />} onClick={() => install(app, app.editions[0])} title={`Install ${app.name} from ${sourceLabel(app.editions[0].package.source)}.`}>
        Install
      </Button>
    );
  }
  return (
    <Dropdown
      name="Install"
      icon={<Download {...ICON} aria-hidden="true" />}
      alone
      form
      align="right"
      className="bs-search-pick"
      options={installOptions(app)}
      value={null}
      onChange={(key) => {
        const edition = app.editions.find((e) => editionKey(e) === key);
        if (edition) install(app, edition);
      }}
    />
  );
}

function RowFigure({ app }: { app: App }) {
  const primary = primaryEdition(app);
  const version = primary?.package.installed_version ?? primary?.package.version ?? null;
  const label = app.editions.map((e) => e.package.popularity_label).find((l): l is string => Boolean(l)) ?? null;
  if (!version && !label) return null;
  return (
    <span className="bs-search-fig">
      {version ? <Figure title={primary?.package.installed_version ? "The installed version." : "The version the source offers."}>{version}</Figure> : null}
      {label ? <span className="bs-search-fig-label">{label}</span> : null}
    </span>
  );
}

export function SearchPage() {
  const settings = useShell((s) => s.settings);
  const statuses = useShell((s) => s.sources);
  const openApp = useShell((s) => s.openApp);

  const text = useSearch((s) => s.text);
  const sources = useSearch((s) => s.sources);
  const kind = useSearch((s) => s.kind);
  const sort = useSearch((s) => s.sort);
  const installedOnly = useSearch((s) => s.installedOnly);
  const results = useSearch((s) => s.results);
  const searched = useSearch((s) => s.searched);
  const loading = useSearch((s) => s.loading);
  const error = useSearch((s) => s.error);
  const selected = useSearch((s) => s.selected);
  const init = useSearch((s) => s.init);
  const setText = useSearch((s) => s.setText);
  const setSources = useSearch((s) => s.setSources);
  const setKind = useSearch((s) => s.setKind);
  const setSort = useSearch((s) => s.setSort);
  const setInstalledOnly = useSearch((s) => s.setInstalledOnly);
  const setSelected = useSearch((s) => s.setSelected);

  const busy = useBusyRefs();
  const inputRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLDivElement>(null);

  // The filters start from settings, once; after that the page owns them.
  useEffect(() => {
    if (settings && statuses.length > 0) init(settings, statuses);
  }, [settings, statuses, init]);

  const sourceOptions = useMemo<DropdownOption<SourceKind>[]>(
    () =>
      statuses.map((s) => ({
        value: s.kind,
        label: sourceLabel(s.kind),
        figure: s.detail ?? undefined,
        disabled: !s.available,
        disabledReason: s.reason ?? undefined,
      })),
    [statuses],
  );

  // Filtering and sorting are the page's own work on the result set.
  const { visible, hiddenPackages, hiddenNotInstalled } = useMemo(() => {
    const apps = results?.apps ?? [];
    const byInstalled = installedOnly ? apps.filter((a) => a.installed) : apps;
    const byKind = kind === "app" ? byInstalled.filter((a) => a.kind === "app") : byInstalled;
    return {
      visible: sortApps(byKind, sort),
      hiddenPackages: byInstalled.length - byKind.length,
      hiddenNotInstalled: apps.length - byInstalled.length,
    };
  }, [results, installedOnly, kind, sort]);

  const visibleRef = useRef(visible);
  visibleRef.current = visible;

  const open = (app: App, index: number) => {
    rememberApp(app);
    setSelected(index);
    openApp(app.key);
  };

  const openSelected = () => {
    const list = visibleRef.current;
    const index = useSearch.getState().selected;
    const app = list[index] ?? list[0];
    if (app) open(app, list.indexOf(app));
  };

  // Ctrl+F and "/" find the field, arrows walk the rows while the field keeps
  // focus, Escape clears. Menus and dialogs handle their own keys first.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.defaultPrevented || document.querySelector(".bs-dimmer, .bs-menu")) return;
      const target = e.target as HTMLElement | null;
      const input = inputRef.current;
      const inField = target !== null && target === input;
      const editable = target !== null && (target.tagName === "INPUT" || target.tagName === "TEXTAREA" || target.isContentEditable);
      if ((e.ctrlKey || e.metaKey) && !e.altKey && e.key.toLowerCase() === "f") {
        e.preventDefault();
        input?.focus();
        input?.select();
        return;
      }
      if (e.key === "/" && !editable) {
        e.preventDefault();
        input?.focus();
        input?.select();
        return;
      }
      if (editable && !inField) return;
      if (e.key === "ArrowDown" || e.key === "ArrowUp") {
        const count = visibleRef.current.length;
        if (count === 0) return;
        e.preventDefault();
        const current = useSearch.getState().selected;
        const next = e.key === "ArrowDown" ? Math.min(count - 1, current + 1) : Math.max(0, current - 1);
        setSelected(next);
        listRef.current?.children[next]?.scrollIntoView({ block: "nearest" });
        return;
      }
      if (e.key === "Escape") {
        if (useSearch.getState().text) setText("");
        input?.focus();
      }
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [setSelected, setText]);

  const availableLabels = statuses.filter((s) => s.available).map((s) => sourceLabel(s.kind));
  const searchedLabels = (results?.searched ?? []).map(sourceLabel);
  const typed = text.trim().length >= 2;

  let body: ReactNode;
  if (!typed) {
    body = (
      <EmptyState fill icon={<Search {...ICON_EMPTY} aria-hidden="true" />}>
        {availableLabels.length > 0 ? `Type to search ${joinAnd(availableLabels)}.` : "Type to search. No source is available on this machine yet."}
      </EmptyState>
    );
  } else if (loading && !results) {
    body = (
      <div className="bs-well">
        <SkeletonAppRows count={8} />
      </div>
    );
  } else if (!results) {
    body = null;
  } else if (visible.length === 0) {
    const where = searchedLabels.length > 0 ? joinAnd(searchedLabels) : null;
    let more: ReactNode = null;
    let action: ReactNode = null;
    if (searched && searched.sources.length === 0) {
      more = "No source is ticked. Choose one under Sources.";
    } else if (kind === "app" && hiddenPackages > 0) {
      more = (
        <>
          <Figure>{formatCount(hiddenPackages)}</Figure> {hiddenPackages === 1 ? "package matches" : "packages match"}; switch to All packages to see {hiddenPackages === 1 ? "it" : "them"}.
        </>
      );
      action = <Button onClick={() => setKind("all")}>Show all packages</Button>;
    } else if (installedOnly && hiddenNotInstalled > 0) {
      more = (
        <>
          <Figure>{formatCount(hiddenNotInstalled)}</Figure> {hiddenNotInstalled === 1 ? "result is" : "results are"} not installed; turn off Installed only to see {hiddenNotInstalled === 1 ? "it" : "them"}.
        </>
      );
      action = <Button onClick={() => setInstalledOnly(false)}>Show every result</Button>;
    }
    body = (
      <EmptyState fill icon={<Search {...ICON_EMPTY} aria-hidden="true" />} action={action}>
        {where ? `Nothing matches "${searched?.text ?? text.trim()}" in ${where}.` : `Nothing matches "${searched?.text ?? text.trim()}".`}
        {more ? (
          <>
            <br />
            {more}
          </>
        ) : null}
      </EmptyState>
    );
  } else {
    body = (
      <div className="bs-well">
        <div className="bs-list" ref={listRef} aria-busy={loading || undefined}>
          {visible.map((app, i) => (
            <AppRow
              key={app.key}
              app={app}
              selected={i === selected}
              onOpen={(a) => open(a, i)}
              figure={<RowFigure app={app} />}
              action={<RowAction app={app} busy={busy} />}
            />
          ))}
        </div>
      </div>
    );
  }

  return (
    <div className="bs-search">
      <div className="bs-toolbar">
        <span className="bs-toolbar-title">Search</span>
        <SearchField
          label="Search"
          value={text}
          onChange={setText}
          placeholder="Search applications"
          width="240px"
          autoFocus
          inputRef={inputRef}
          onSubmit={openSelected}
        />
        <MultiSelect name="Sources" options={sourceOptions} values={sources} onChange={setSources} alone />
        <Segmented name="Kind" options={KINDS} value={kind} onChange={setKind} />
        <Dropdown name="Sort" icon={<ArrowDownUp {...ICON} aria-hidden="true" />} options={SORTS} value={sort} onChange={setSort} />
        <div className="bs-toolbar-end">
          <span className="bs-search-toggle">
            Installed only
            <Toggle label="Installed only" on={installedOnly} onChange={setInstalledOnly} />
          </span>
        </div>
      </div>
      <div className="bs-search-body">
        <div className="bs-page">
          {error || (results && results.failed.length > 0) ? (
            <div className="bs-search-notices">
              {error ? <Notice>{error}</Notice> : null}
              {results?.failed.map(([source, sentence]) => (
                <Notice key={source}>{sentence.toLowerCase().includes(sourceLabel(source).toLowerCase()) ? sentence : `${sourceLabel(source)}: ${sentence}`}</Notice>
              ))}
            </div>
          ) : null}
          {typed && results && visible.length > 0 ? (
            <div className="bs-search-line" role="status" aria-live="polite">
              {loading ? (
                "Searching…"
              ) : (
                <>
                  <Figure>{formatCount(visible.length)}</Figure> {visible.length === 1 ? "result" : "results"} from {joinAnd(searchedLabels)}
                  {kind === "app" && hiddenPackages > 0 ? (
                    <>
                      {" · "}
                      <Figure>{formatCount(hiddenPackages)}</Figure> {hiddenPackages === 1 ? "package" : "packages"} hidden by the Applications filter
                    </>
                  ) : null}
                </>
              )}
            </div>
          ) : null}
          {body}
        </div>
      </div>
    </div>
  );
}
