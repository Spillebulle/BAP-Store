//! Every `#[tauri::command]`, thin. The names are the contract with the
//! page (`frontend/src/api.ts` calls them by these exact strings) and every
//! one returns `Result<T, String>` where the `String` is a sentence the
//! page can show as it is.
//!
//! The commands themselves only move values across the IPC boundary; what
//! they do lives in [`logic`], which knows nothing of Tauri so the text
//! mode and the tests run the same code without a window. Anything that
//! touches the store runs on a blocking worker (`spawn_blocking`): the
//! sources are synchronous by design, and a search over the pacman database
//! on the async runtime's threads would stall every other command.
//!
//! A running transaction reaches the page as `transaction://event` with the
//! [`Event`] as payload, and the same events are kept in the plan's
//! [`PlanStatus`] so a page that was elsewhere when they were sent can catch
//! up through `active_plans`.

pub mod selfupdate_adapter;

pub use selfupdate_adapter::SelfUpdate;

use crate::settings::Settings;
use crate::state::{AppState, PlanStatus};
use bap_core::updates::UpdateList;
use bap_core::{
    DriversReport, Event, Op, Package, PackageRef, Plan, Query, SearchResult, SourceStatus,
    SystemInfo,
};
use serde::{Deserialize, Serialize};
use tauri::ipc::Invoke;

/// The event name the page listens on for a running transaction.
pub const TRANSACTION_EVENT: &str = "transaction://event";

/// What the confirm step shows: the steps that would run and anything the
/// user should know first (a partial upgrade on Arch, a reboot after
/// firmware).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlanPreview {
    pub plan: Plan,
    pub notices: Vec<String>,
}

pub fn handler() -> impl Fn(Invoke<tauri::Wry>) -> bool + Send + Sync + 'static {
    tauri::generate_handler![
        system_info,
        sources,
        search,
        installed,
        app_details,
        updates,
        plan,
        run_plan,
        cancel_plan,
        active_plans,
        drivers,
        settings_get,
        settings_set,
        self_update_check,
        self_update_apply,
        group_split,
    ]
}

/// Run store work on a blocking worker and bring its answer back. A worker
/// that died is reported as a sentence rather than a panic in the page.
async fn blocking<T, F>(work: F) -> Result<T, String>
where
    F: FnOnce() -> Result<T, String> + Send + 'static,
    T: Send + 'static,
{
    match tauri::async_runtime::spawn_blocking(work).await {
        Ok(answer) => answer,
        Err(e) => Err(format!(
            "The store's worker stopped before it answered: {e}. Try again."
        )),
    }
}

/// The page-facing end of a transaction: every event goes out as
/// `transaction://event`. A page that is not listening loses nothing, since
/// the same events are kept on the plan.
fn emitter(app: tauri::AppHandle) -> impl Fn(&Event) + Send + Sync + 'static {
    use tauri::Emitter;
    move |event| {
        if let Err(e) = app.emit(TRANSACTION_EVENT, event) {
            log::warn!("could not send {TRANSACTION_EVENT} to the page: {e}");
        }
    }
}

#[tauri::command]
async fn system_info() -> Result<SystemInfo, String> {
    // Read straight from os-release rather than through the store, so the
    // status bar can paint before the package databases are read.
    blocking(|| Ok(bap_core::system::detect())).await
}

#[tauri::command]
async fn sources(state: tauri::State<'_, AppState>) -> Result<Vec<SourceStatus>, String> {
    let state = state.inner().clone();
    blocking(move || Ok(logic::sources(&state))).await
}

#[tauri::command]
async fn search(state: tauri::State<'_, AppState>, query: Query) -> Result<SearchResult, String> {
    let state = state.inner().clone();
    blocking(move || Ok(logic::search(&state, query))).await
}

#[tauri::command]
async fn installed(state: tauri::State<'_, AppState>) -> Result<SearchResult, String> {
    let state = state.inner().clone();
    blocking(move || Ok(logic::installed(&state))).await
}

#[tauri::command]
async fn app_details(
    state: tauri::State<'_, AppState>,
    refs: Vec<PackageRef>,
) -> Result<Vec<Package>, String> {
    let state = state.inner().clone();
    blocking(move || Ok(logic::app_details(&state, refs))).await
}

#[tauri::command]
async fn updates(state: tauri::State<'_, AppState>, force: bool) -> Result<UpdateList, String> {
    let state = state.inner().clone();
    blocking(move || Ok(logic::updates(&state, force))).await
}

#[tauri::command]
async fn plan(state: tauri::State<'_, AppState>, ops: Vec<Op>) -> Result<PlanPreview, String> {
    let state = state.inner().clone();
    blocking(move || logic::preview(&state, ops)).await
}

#[tauri::command]
async fn run_plan(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    ops: Vec<Op>,
) -> Result<String, String> {
    let state = state.inner().clone();
    let emit = emitter(app);
    blocking(move || logic::run_plan(&state, ops, emit)).await
}

#[tauri::command]
fn cancel_plan(state: tauri::State<'_, AppState>, id: String) -> Result<(), String> {
    state.cancel(&id)
}

#[tauri::command]
fn active_plans(state: tauri::State<'_, AppState>) -> Result<Vec<PlanStatus>, String> {
    Ok(state.plans())
}

#[tauri::command]
async fn drivers(state: tauri::State<'_, AppState>) -> Result<DriversReport, String> {
    let state = state.inner().clone();
    blocking(move || Ok(logic::drivers(&state))).await
}

#[tauri::command]
fn settings_get(state: tauri::State<'_, AppState>) -> Result<Settings, String> {
    Ok(state.settings())
}

#[tauri::command]
fn settings_set(state: tauri::State<'_, AppState>, settings: Settings) -> Result<Settings, String> {
    state.set_settings(settings)
}

#[tauri::command]
async fn self_update_check(
    state: tauri::State<'_, AppState>,
    force: bool,
) -> Result<SelfUpdate, String> {
    let state = state.inner().clone();
    blocking(move || logic::self_update_check(&state, force)).await
}

#[tauri::command]
async fn self_update_apply(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    let state = state.inner().clone();
    let emit = emitter(app);
    blocking(move || logic::self_update_apply(&state, emit)).await
}

#[tauri::command]
fn group_split(state: tauri::State<'_, AppState>, package: PackageRef) -> Result<Settings, String> {
    logic::group_split(&state, package)
}

/// A core error as the page shows it: trimmed, ending in a full stop.
/// Sources already write sentences; this only closes the ones that forgot.
pub fn sentence(message: &str) -> String {
    let text = message.trim();
    if text.is_empty() {
        return "Something went wrong and the source did not say what.".to_string();
    }
    if text.ends_with(['.', '!', '?']) {
        text.to_string()
    } else {
        format!("{text}.")
    }
}

/// What the commands do, with no Tauri in it.
pub mod logic {
    use super::{PlanPreview, SelfUpdate, selfupdate_adapter, sentence};
    use crate::settings::Settings;
    use crate::state::{AppState, PlanState, SELF_UPDATE_MAX_AGE, UPDATES_MAX_AGE};
    use bap_core::transaction::runner::Runner;
    use bap_core::updates::UpdateList;
    use bap_core::{
        DriversReport, Event, Op, Package, PackageRef, Plan, Query, SearchResult, SourceStatus,
        Store,
    };
    use std::sync::Arc;

    /// The most results a source is asked for. Above this the grouper and
    /// the page are sorting rows nobody scrolls to.
    pub const MAX_LIMIT: usize = 300;
    /// What a query that names no limit gets; the same figure as
    /// `Query::new`.
    pub const DEFAULT_LIMIT: usize = 200;

    pub fn sources(state: &AppState) -> Vec<SourceStatus> {
        state.store().statuses()
    }

    /// The query as the store sees it: the user's enabled sources when the
    /// page named none, and a limit the sources can honour.
    pub fn shape_query(mut query: Query, settings: &Settings) -> Query {
        if query.sources.is_none() {
            query.sources = Some(settings.enabled_sources.clone());
        }
        if query.split.is_empty() {
            query.split = settings.split.clone();
        }
        query.limit = match query.limit {
            0 => DEFAULT_LIMIT,
            n => n.min(MAX_LIMIT),
        };
        query
    }

    pub fn search(state: &AppState, query: Query) -> SearchResult {
        let query = shape_query(query, &state.settings());
        state.store().search(&query)
    }

    pub fn installed(state: &AppState) -> SearchResult {
        state.store().installed()
    }

    /// The full record for each reference, asked for in parallel. A source
    /// that fails costs its own reference and a log line, never the call:
    /// the detail page draws what it got.
    pub fn app_details(state: &AppState, refs: Vec<PackageRef>) -> Vec<Package> {
        let store = state.store();
        std::thread::scope(|scope| {
            let handles: Vec<_> = refs
                .iter()
                .map(|r| {
                    let store = &store;
                    scope.spawn(move || details_one(store, r))
                })
                .collect();
            handles
                .into_iter()
                .filter_map(|h| {
                    h.join().unwrap_or_else(|_| {
                        log::warn!(
                            "a source panicked while answering details; its package is left out"
                        );
                        None
                    })
                })
                .collect()
        })
    }

    fn details_one(store: &Store, r: &PackageRef) -> Option<Package> {
        let Some(source) = store.source(r.source) else {
            log::warn!(
                "details asked of {}, which is not a source on this machine",
                r.source.label()
            );
            return None;
        };
        let status = source.status();
        if !status.available {
            log::warn!(
                "details asked of {} for {} but it is unavailable: {}",
                r.source.label(),
                r.id,
                status.reason.unwrap_or_default()
            );
            return None;
        }
        match source.details(&r.id) {
            Ok(p) => Some(p),
            Err(e) => {
                log::warn!(
                    "{} could not give details for {}: {}",
                    r.source.label(),
                    r.id,
                    e.message
                );
                None
            }
        }
    }

    /// The merged update list, from the cache when it is under ten minutes
    /// old and the page did not insist. A forced check, and the first check
    /// of a session, ask the sources to refresh their indexes first (root
    /// free), so what the page shows is what the machine would get. Only
    /// the first: a finished transaction empties the cache, and the check
    /// after it reads the databases the transaction just wrote rather than
    /// fetching them all again.
    pub fn updates(state: &AppState, force: bool) -> UpdateList {
        if !force && let Some(list) = state.cached_updates(UPDATES_MAX_AGE) {
            return list;
        }
        let store = state.store();
        let list = if force || !state.updates_refreshed_once() {
            state.mark_updates_refreshed();
            store.updates_refreshed()
        } else {
            store.updates()
        };
        state.store_updates(list.clone());
        list
    }

    pub fn drivers(state: &AppState) -> DriversReport {
        bap_core::drivers::report(&state.store())
    }

    /// A dry run for the confirm step.
    pub fn preview(state: &AppState, ops: Vec<Op>) -> Result<PlanPreview, String> {
        let store = state.store();
        let plan = store.plan(&ops).map_err(|e| sentence(&e.message))?;
        let notices = notices(&store, &ops);
        Ok(PlanPreview { plan, notices })
    }

    /// What the user should read before confirming: the partial-upgrade
    /// notice on Arch, and whatever else the plan builder knows.
    pub fn notices(store: &Store, ops: &[Op]) -> Vec<String> {
        bap_core::transaction::notices(store, ops)
    }

    /// Build the plan and start it. The id comes back at once; everything
    /// after that arrives as events.
    pub fn run_plan(
        state: &AppState,
        ops: Vec<Op>,
        emit: impl Fn(&Event) + Send + Sync + 'static,
    ) -> Result<String, String> {
        let store = state.store();
        let plan = store.plan(&ops).map_err(|e| sentence(&e.message))?;
        if plan.steps.is_empty() {
            return Err(
                "There is nothing to do. Everything asked for is already in place.".to_string(),
            );
        }
        Ok(start_plan(state, store, plan, emit))
    }

    /// Record the plan and run it on a thread of its own. The thread holds a
    /// clone of the state, not a borrow, so it outlives the command that
    /// started it, and the store the plan was built from, so the sources
    /// that built it are the ones told how it ended even when another
    /// plan has reset the state's store in the meantime. Every event is
    /// appended to the plan's record before it is sent, so a page that
    /// reacts to an event by asking `active_plans` already sees it there.
    pub fn start_plan(
        state: &AppState,
        store: Arc<Store>,
        plan: Plan,
        emit: impl Fn(&Event) + Send + Sync + 'static,
    ) -> String {
        let id = plan.id.clone();
        state.add_plan(plan.clone());
        // The runner is made here, before the thread, so its cancel token
        // is on record by the time the id reaches the page.
        let runner = Runner::new();
        state.set_cancel_token(&id, runner.cancel_token());
        let emit: Arc<dyn Fn(&Event) + Send + Sync> = Arc::new(emit);
        let worker_state = state.clone();
        let worker_emit = emit.clone();
        let spawned = std::thread::Builder::new()
            .name(format!("plan {id}"))
            .spawn(move || {
                run_to_completion(&worker_state, &store, &runner, &plan, worker_emit.as_ref())
            });
        if let Err(e) = spawned {
            // No thread means no runner, so settle the plan here rather than
            // leave the panel waiting on something nobody is running.
            let event = Event::PlanFinished {
                plan: id.clone(),
                ok: false,
                message: format!("Could not start a worker for the plan: {e}. Try again."),
            };
            state.push_event(&id, event.clone());
            emit(&event);
        }
        id
    }

    /// What happens to every event a plan produces: recorded on the plan,
    /// acted on, then sent to the page, in that order. `store` is the one
    /// the plan was built from, not the state's current one, which another
    /// plan may have reset since.
    pub fn deliver(
        state: &AppState,
        store: &Store,
        id: &str,
        event: Event,
        emit: &(dyn Fn(&Event) + Send + Sync),
    ) {
        let finished = match &event {
            Event::PlanFinished { ok, .. } => Some(*ok),
            _ => None,
        };
        state.push_event(id, event.clone());
        if let Some(ok) = finished
            && let Some(ops) = state.plan_ops(id)
        {
            // The sources that keep their own records (GitHub) learn how it
            // went before the state's store is dropped below.
            store.finished(&ops, ok);
        }
        if finished == Some(true) {
            // The machine changed under the sources. The store is detected
            // again on the next command; the sources reload by mtime
            // anyway, but `Store::detect` also re-runs availability, which
            // is what makes a newly installed Flatpak appear without a
            // restart.
            state.invalidate_after_transaction();
        }
        emit(&event);
    }

    fn run_to_completion(
        state: &AppState,
        store: &Store,
        runner: &Runner,
        plan: &Plan,
        emit: &(dyn Fn(&Event) + Send + Sync),
    ) {
        let id = plan.id.clone();
        let mut sink = |event: Event| deliver(state, store, &id, event, emit);
        let outcome = runner.execute(plan, &mut sink);
        // The runner ends every run with PlanFinished; this only guards
        // the invariant the panel relies on, so it never waits for ever.
        if state.plan_state(&id) == Some(PlanState::Running) {
            sink(Event::PlanFinished {
                plan: id.clone(),
                ok: outcome.ok,
                message: sentence(&outcome.message),
            });
        }
    }

    /// Whether a newer BAP Store exists. Cached for an hour in memory and
    /// six hours on disk; a forced check (the button) goes past both to
    /// GitHub, an unforced one (start-up) reads the caches and asks only
    /// when the setting allows it, because that is what the setting
    /// promises.
    pub fn self_update_check(state: &AppState, force: bool) -> Result<SelfUpdate, String> {
        if !force && let Some(cached) = state.cached_self_update(SELF_UPDATE_MAX_AGE) {
            return Ok(cached);
        }
        if !force && !state.settings().self_update_check {
            return Ok(selfupdate_adapter::unchecked());
        }
        let result = state.check_self_update(force)?;
        state.store_self_update(result.clone());
        Ok(result)
    }

    /// Apply the remedy from the last check, checking first, past every
    /// cache, if there was none this hour.
    pub fn self_update_apply(
        state: &AppState,
        emit: impl Fn(&Event) + Send + Sync + 'static,
    ) -> Result<String, String> {
        let update = match state.cached_self_update(SELF_UPDATE_MAX_AGE) {
            Some(u) => u,
            None => {
                let u = state.check_self_update(true)?;
                state.store_self_update(u.clone());
                u
            }
        };
        let plan = selfupdate_adapter::plan(&update)?;
        if plan.steps.is_empty() {
            return Err("The update has no steps to run on this machine. The check says how to get it instead.".to_string());
        }
        Ok(start_plan(state, state.store(), plan, emit))
    }

    /// Remember that this edition does not belong to its group.
    pub fn group_split(state: &AppState, package: PackageRef) -> Result<Settings, String> {
        let mut settings = state.settings();
        settings.add_split(package.source, &package.id);
        state.set_settings(settings)
    }
}

#[cfg(test)]
mod tests {
    // The logic functions share names with the Tauri wrappers, so only the
    // logic side is glob-imported and the rest is named.
    use super::logic::*;
    use super::{PlanPreview, selfupdate_adapter, sentence};
    use crate::settings::Settings;
    use crate::state::{AppState, PlanState, SelfUpdateChecker};
    use bap_core::updates::UpdateList;
    use bap_core::{
        Event, Op, Package, PackageRef, Plan, Query, Source, SourceKind, SourceStatus, Step, Store,
        Update,
    };
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex, mpsc};
    use std::time::Duration;

    fn scratch_path(name: &str) -> std::path::PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir()
            .join(format!(
                "bap-store-cmd-{}-{nanos}-{name}",
                std::process::id()
            ))
            .join(crate::settings::FILE_NAME)
    }

    /// A state over the machine's own sources, for the tests whose subject
    /// is the plan machinery rather than a source.
    fn scratch_state(name: &str) -> AppState {
        AppState::at(scratch_path(name))
    }

    /// A store with no sources: nothing is read, nothing is fetched.
    fn empty_store() -> Store {
        Store {
            system: bap_core::system::from_os_release(""),
            sources: Vec::new(),
        }
    }

    /// A state over `store`, so a test never detects the machine's sources.
    fn state_with(name: &str, store: Store) -> AppState {
        AppState::with_store(scratch_path(name), store)
    }

    /// A source that answers nothing and counts what it was asked.
    #[derive(Default)]
    struct Counts {
        refreshes: AtomicUsize,
        finished: Mutex<Vec<(Op, bool)>>,
    }

    struct Fake {
        kind: SourceKind,
        counts: Arc<Counts>,
    }

    impl Fake {
        fn store(kind: SourceKind) -> (Store, Arc<Counts>) {
            let counts = Arc::new(Counts::default());
            let store = Store {
                system: bap_core::system::from_os_release(""),
                sources: vec![Box::new(Fake {
                    kind,
                    counts: counts.clone(),
                })],
            };
            (store, counts)
        }
    }

    impl Source for Fake {
        fn kind(&self) -> SourceKind {
            self.kind
        }
        fn status(&self) -> SourceStatus {
            SourceStatus {
                kind: self.kind,
                available: true,
                reason: None,
                detail: None,
            }
        }
        fn search(&self, _query: &Query) -> bap_core::Result<Vec<Package>> {
            Ok(Vec::new())
        }
        fn installed(&self) -> bap_core::Result<Vec<Package>> {
            Ok(Vec::new())
        }
        fn updates(&self) -> bap_core::Result<Vec<Update>> {
            Ok(Vec::new())
        }
        fn details(&self, id: &str) -> bap_core::Result<Package> {
            Err(bap_core::Error::new(format!("{id} is not known.")))
        }
        fn plan(&self, _op: &Op) -> bap_core::Result<Vec<Step>> {
            Ok(Vec::new())
        }
        fn refresh_index(&self) -> bap_core::Result<()> {
            self.counts.refreshes.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
        fn finished(&self, op: &Op, ok: bool) {
            self.counts
                .finished
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .push((op.clone(), ok));
        }
    }

    fn steam() -> PackageRef {
        PackageRef {
            source: SourceKind::Pacman,
            id: "steam".into(),
        }
    }

    #[test]
    fn a_sentence_ends_with_a_full_stop_and_nothing_is_said_twice() {
        assert_eq!(
            sentence("could not reach aur.archlinux.org"),
            "could not reach aur.archlinux.org."
        );
        assert_eq!(
            sentence("Flatpak is not installed. "),
            "Flatpak is not installed."
        );
        assert_eq!(sentence("Really?"), "Really?");
        assert!(sentence("   ").contains("did not say what"));
    }

    #[test]
    fn a_query_takes_the_enabled_sources_and_a_sane_limit() {
        let settings = Settings {
            enabled_sources: vec![SourceKind::Pacman, SourceKind::Aur],
            ..Settings::default()
        };

        let q = shape_query(Query::new("steam"), &settings);
        assert_eq!(q.sources, Some(vec![SourceKind::Pacman, SourceKind::Aur]));
        assert_eq!(q.limit, DEFAULT_LIMIT);

        let explicit = Query {
            text: "steam".into(),
            sources: Some(vec![SourceKind::Flatpak]),
            limit: 5000,
            split: Vec::new(),
        };
        let q = shape_query(explicit, &settings);
        assert_eq!(
            q.sources,
            Some(vec![SourceKind::Flatpak]),
            "the page's own choice stands"
        );
        assert_eq!(q.limit, MAX_LIMIT);

        let zero = Query {
            text: "x".into(),
            sources: None,
            limit: 0,
            split: Vec::new(),
        };
        assert_eq!(shape_query(zero, &settings).limit, DEFAULT_LIMIT);
    }

    #[test]
    fn details_leaves_out_what_failed_and_never_fails_itself() {
        let state = scratch_state("details");
        // A name no source has, and a Flatpak ref on a machine that may
        // have no Flatpak: each costs its own reference and nothing else.
        let got = app_details(
            &state,
            vec![
                PackageRef {
                    source: SourceKind::Pacman,
                    id: "no-such-package-bap-store-test".into(),
                },
                PackageRef {
                    source: SourceKind::Flatpak,
                    id: "nowhere/app/io.example.NoSuchApp/x86_64/stable".into(),
                },
            ],
        );
        assert!(got.is_empty());
        assert!(app_details(&state, Vec::new()).is_empty());
    }

    #[test]
    fn updates_come_from_the_cache_unless_forced() {
        let state = state_with("updates", empty_store());
        state.store_updates(UpdateList {
            checked_at: 42,
            ..UpdateList::default()
        });
        assert_eq!(updates(&state, false).checked_at, 42);
        let fresh = updates(&state, true);
        assert_ne!(fresh.checked_at, 42, "a forced check asks the sources");
        assert_eq!(
            updates(&state, false).checked_at,
            fresh.checked_at,
            "and is cached in turn"
        );
    }

    #[test]
    fn the_index_refresh_runs_once_a_session_and_again_only_when_forced() {
        let (store, counts) = Fake::store(SourceKind::Pacman);
        let state = state_with("refresh-once", store);
        let refreshes = || counts.refreshes.load(Ordering::SeqCst);

        updates(&state, false);
        assert_eq!(refreshes(), 1, "the first check of a session refreshes");
        updates(&state, false);
        assert_eq!(refreshes(), 1, "the second is answered from the cache");

        // A finished transaction empties the update cache (and forgets the
        // store, which `state.rs` covers: the flag survives that too); the
        // check after it reads what is on disk rather than fetching every
        // package list again.
        state.invalidate_updates();
        updates(&state, false);
        assert_eq!(refreshes(), 1, "no refresh after a transaction");
        updates(&state, true);
        assert_eq!(refreshes(), 2, "a forced check always refreshes");
    }

    #[test]
    fn a_preview_is_a_dry_run_with_notices() {
        let state = scratch_state("preview");
        let preview = preview(&state, vec![Op::Install { package: steam() }]).unwrap();
        assert!(preview.plan.id.starts_with("plan-"));
        assert_eq!(preview.plan.ops.len(), 1);
        assert!(preview.notices.is_empty(), "nothing to say on this branch");
        assert!(
            state.plans().is_empty(),
            "a preview runs nothing and records nothing"
        );
    }

    #[test]
    fn a_plan_with_nothing_to_do_is_refused_before_it_starts() {
        let state = scratch_state("empty");
        // Refreshing the AUR is a no-op by design (its index is the RPC), so
        // the plan is empty on every machine.
        let err = run_plan(
            &state,
            vec![Op::Refresh {
                source: SourceKind::Aur,
            }],
            |_| {},
        )
        .unwrap_err();
        assert!(err.starts_with("There is nothing to do."), "{err}");
        assert!(state.plans().is_empty());
    }

    #[test]
    fn a_started_plan_settles_and_tells_the_page_when_the_runner_stops() {
        let state = state_with("start", empty_store());
        let plan = Plan {
            id: "plan-test".into(),
            ops: vec![Op::Install { package: steam() }],
            // A session step that fails at once: no helper, no prompt, and
            // the runner still has to end the plan with a sentence.
            steps: vec![bap_core::Step {
                source: SourceKind::Pacman,
                title: "Failing on purpose".into(),
                command: bap_core::Command {
                    program: "sh".into(),
                    args: vec!["-c".into(), "exit 3".into()],
                    env: Vec::new(),
                    cwd: None,
                },
                needs_root: false,
                weight: 1,
            }],
        };
        let (tx, rx) = mpsc::channel::<Event>();
        let id = start_plan(&state, state.store(), plan, move |e| {
            let _ = tx.send(e.clone());
        });
        assert_eq!(id, "plan-test");

        // The step fails; the plan must still end with a PlanFinished the
        // page can act on, after whatever came before it.
        let finished = loop {
            let event = rx
                .recv_timeout(Duration::from_secs(10))
                .expect("the page is told");
            if matches!(event, Event::PlanFinished { .. }) {
                break event;
            }
        };
        match &finished {
            Event::PlanFinished { plan, ok, message } => {
                assert_eq!(plan, "plan-test");
                assert!(!ok);
                assert!(message.ends_with('.'), "a sentence: {message}");
            }
            other => panic!("expected PlanFinished, got {other:?}"),
        }
        // Wait for the worker to record the state, which it does before it sends.
        assert_eq!(state.plan_state("plan-test"), Some(PlanState::Failed));
        let status = &state.plans()[0];
        assert_eq!(
            status.events.last(),
            Some(&finished),
            "the record and the page agree"
        );
        assert!(status.started > 0);
    }

    #[test]
    fn a_successful_plan_forgets_the_store_and_the_update_list() {
        let state = state_with("invalidate", empty_store());
        let plan = Plan {
            id: "plan-ok".into(),
            ops: Vec::new(),
            steps: Vec::new(),
        };
        let store = state.store();
        state.store_updates(UpdateList::default());
        state.add_plan(plan);
        // The sink's policy is what is under test, so drive it the way the
        // runner would rather than through a runner that cannot succeed here.
        let (tx, rx) = mpsc::channel::<Event>();
        let emit = move |e: &Event| {
            let _ = tx.send(e.clone());
        };
        deliver(
            &state,
            &store,
            "plan-ok",
            Event::Log {
                plan: "plan-ok".into(),
                step: 0,
                line: "resolving dependencies...".into(),
                stderr: false,
            },
            &emit,
        );
        assert!(state.has_store(), "a log line changes nothing");
        deliver(
            &state,
            &store,
            "plan-ok",
            Event::PlanFinished {
                plan: "plan-ok".into(),
                ok: true,
                message: "Installed.".into(),
            },
            &emit,
        );
        assert_eq!(state.plan_state("plan-ok"), Some(PlanState::Done));
        assert!(
            !state.has_store(),
            "the store is detected again on the next command"
        );
        assert!(
            state.cached_updates(Duration::from_secs(600)).is_none(),
            "the update list is stale"
        );
        assert_eq!(rx.try_iter().count(), 2, "both events reached the page");
    }

    /// The sources that built a plan are told how it ended, even when
    /// another plan finished first and reset the state's store: a GitHub
    /// install record would otherwise be lost.
    #[test]
    fn the_sources_that_built_a_plan_are_told_how_it_ended_after_a_reset() {
        let (store, counts) = Fake::store(SourceKind::Github);
        let state = state_with("finished", store);
        let op = Op::Install {
            package: PackageRef {
                source: SourceKind::Github,
                id: "sharkdp/bat".into(),
            },
        };
        let bound = state.store();
        state.add_plan(Plan {
            id: "plan-b".into(),
            ops: vec![op.clone()],
            steps: Vec::new(),
        });
        // Plan A finishes first and resets the store under plan B.
        state.invalidate_after_transaction();
        assert!(!state.has_store());
        deliver(
            &state,
            &bound,
            "plan-b",
            Event::PlanFinished {
                plan: "plan-b".into(),
                ok: true,
                message: "Installed.".into(),
            },
            &|_| {},
        );
        let finished = counts.finished.lock().unwrap();
        assert_eq!(finished.as_slice(), &[(op, true)]);
    }

    #[test]
    fn a_self_update_check_respects_the_setting_unless_forced() {
        // A checker that answers from a table and remembers whether it was
        // asked to go past the disk cache, so GitHub is never asked here.
        let asked: Arc<Mutex<Vec<bool>>> = Arc::new(Mutex::new(Vec::new()));
        let seen = asked.clone();
        let checker: SelfUpdateChecker = Arc::new(move |fresh: bool| {
            seen.lock().unwrap().push(fresh);
            Ok(selfupdate_adapter::unchecked())
        });
        let state =
            AppState::with_store_and_checker(scratch_path("selfupdate"), empty_store(), checker);
        let mut settings = state.settings();
        settings.self_update_check = false;
        state.set_settings(settings).unwrap();

        let unforced = self_update_check(&state, false).unwrap();
        assert_eq!(unforced.current, env!("CARGO_PKG_VERSION"));
        assert!(
            state
                .cached_self_update(Duration::from_secs(3600))
                .is_none(),
            "nothing was asked, so nothing was cached"
        );
        assert!(asked.lock().unwrap().is_empty(), "the setting was honoured");

        let forced = self_update_check(&state, true).unwrap();
        assert_eq!(forced.current, env!("CARGO_PKG_VERSION"));
        assert!(
            state
                .cached_self_update(Duration::from_secs(3600))
                .is_some()
        );
        assert_eq!(
            asked.lock().unwrap().as_slice(),
            &[true],
            "a forced check goes past the disk cache"
        );

        let err = self_update_apply(&state, |_| {}).unwrap_err();
        assert!(err.starts_with("BAP Store is up to date"), "{err}");
        assert_eq!(
            asked.lock().unwrap().len(),
            1,
            "apply used the check from a moment ago"
        );
    }

    #[test]
    fn splitting_a_group_is_remembered_once() {
        let state = scratch_state("split");
        let first = group_split(&state, steam()).unwrap();
        let second = group_split(&state, steam()).unwrap();
        assert_eq!(first.split, vec!["pacman:steam".to_string()]);
        assert_eq!(second.split, first.split);
        assert_eq!(state.settings().split, first.split);
    }

    #[test]
    fn the_preview_serialises_with_its_two_fields() {
        let json = serde_json::to_value(PlanPreview {
            plan: Plan {
                id: "p".into(),
                ops: Vec::new(),
                steps: Vec::new(),
            },
            notices: vec!["A notice.".into()],
        })
        .unwrap();
        assert_eq!(json["plan"]["id"], "p");
        assert_eq!(json["notices"][0], "A notice.");
    }
}
