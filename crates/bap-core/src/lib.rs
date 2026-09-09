//! BAP Store's library: every source, the metadata, the grouping, the plans
//! and the rules. No window, no root, no global state. `docs/architecture.md`
//! is the design; `CLAUDE.md` lists the invariants.
//!
//! The shape in one paragraph: a [`Source`] answers questions (search,
//! installed, updates, details) and describes how an operation would be
//! carried out as [`Step`]s, but never runs anything. A [`Store`] holds every
//! source this machine can use, fans a query out across them, and groups the
//! results into [`App`]s. A [`Plan`] is what the transaction runner executes,
//! sending the root steps to the helper and running the rest in the session.

pub mod appstream;
pub mod group;
pub mod http;
pub mod model;
pub mod selfupdate;
pub mod sources;
pub mod system;
pub mod transaction;
pub mod updates;
pub mod vercmp;

pub use model::*;

use std::fmt;

/// What went wrong, in a sentence a user can act on. Sources wrap their
/// underlying errors with `context` so the page never shows a bare I/O
/// error.
#[derive(Debug)]
pub struct Error {
    pub message: String,
    pub source_kind: Option<SourceKind>,
}

impl Error {
    pub fn new(message: impl Into<String>) -> Error {
        Error {
            message: message.into(),
            source_kind: None,
        }
    }
    pub fn from_source(kind: SourceKind, message: impl Into<String>) -> Error {
        Error {
            message: message.into(),
            source_kind: Some(kind),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for Error {}

impl From<anyhow::Error> for Error {
    fn from(e: anyhow::Error) -> Error {
        Error::new(format!("{e:#}"))
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Error {
        Error::new(e.to_string())
    }
}

pub type Result<T> = std::result::Result<T, Error>;

/// A search, as the page sends it.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Query {
    pub text: String,
    /// `None` means every available source.
    pub sources: Option<Vec<SourceKind>>,
    /// Per source. Sources return their best matches first.
    pub limit: usize,
}

impl Query {
    pub fn new(text: impl Into<String>) -> Query {
        Query {
            text: text.into(),
            sources: None,
            limit: 200,
        }
    }
}

/// What a search returns: the grouped rows, and the sources that failed with
/// the sentence they failed with. A failed source is never silent.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SearchResult {
    pub apps: Vec<App>,
    pub failed: Vec<(SourceKind, String)>,
    /// Which sources were asked, so the page can say "searched pacman, AUR, Flatpak".
    pub searched: Vec<SourceKind>,
}

/// One place software comes from. Every method is synchronous and may block
/// on the network or the disk; the application runs them on worker threads.
/// A source **never runs anything**: `plan` describes, the runner executes.
pub trait Source: Send + Sync {
    fn kind(&self) -> SourceKind;

    /// Whether this machine can use the source, with a reason when it cannot.
    fn status(&self) -> SourceStatus;

    /// The best matches for the query, most relevant first, at most
    /// `query.limit`. Sources fill `installed` and `installed_version`
    /// themselves so a search result is complete without a second call.
    fn search(&self, query: &Query) -> Result<Vec<Package>>;

    /// Everything this source has installed on the machine.
    fn installed(&self) -> Result<Vec<Package>>;

    /// Everything this source could bring up to date.
    fn updates(&self) -> Result<Vec<Update>>;

    /// The full record for one thing, including what search leaves out
    /// (description, screenshots, facts).
    fn details(&self, id: &str) -> Result<Package>;

    /// How the operation would be carried out. An empty list means the
    /// source has nothing to do for it (already installed, nothing to update).
    fn plan(&self, op: &Op) -> Result<Vec<Step>>;
}

/// The sources this machine has, and the operations across them.
pub struct Store {
    pub system: SystemInfo,
    pub sources: Vec<Box<dyn Source>>,
}

impl Store {
    /// Build every source the machine could have, in interface order. Each
    /// reports its own availability; unavailable sources stay in the list so
    /// the page can say why.
    pub fn detect() -> Store {
        let system = system::detect();
        let client = http::Client::shared();
        let catalogue = appstream::Catalogue::load_system(&system);
        let sources = sources::all(&system, client, catalogue);
        Store { system, sources }
    }

    pub fn statuses(&self) -> Vec<SourceStatus> {
        self.sources.iter().map(|s| s.status()).collect()
    }

    pub fn source(&self, kind: SourceKind) -> Option<&dyn Source> {
        self.sources.iter().find(|s| s.kind() == kind).map(|s| s.as_ref())
    }

    fn selected(&self, wanted: &Option<Vec<SourceKind>>) -> Vec<&dyn Source> {
        self.sources
            .iter()
            .map(|s| s.as_ref())
            .filter(|s| s.status().available)
            .filter(|s| wanted.as_ref().is_none_or(|w| w.contains(&s.kind())))
            .collect()
    }

    /// Search every selected, available source at once and group the results.
    pub fn search(&self, query: &Query) -> SearchResult {
        let sources = self.selected(&query.sources);
        let searched: Vec<SourceKind> = sources.iter().map(|s| s.kind()).collect();
        let results: Vec<(SourceKind, Result<Vec<Package>>)> = std::thread::scope(|scope| {
            let handles: Vec<_> = sources
                .iter()
                .map(|s| {
                    let kind = s.kind();
                    scope.spawn(move || (kind, s.search(query)))
                })
                .collect();
            handles.into_iter().map(|h| h.join().expect("a source panicked")).collect()
        });
        let mut packages = Vec::new();
        let mut failed = Vec::new();
        for (kind, result) in results {
            match result {
                Ok(mut found) => packages.append(&mut found),
                Err(e) => failed.push((kind, e.message)),
            }
        }
        let apps = group::group(packages, &query.text);
        SearchResult {
            apps,
            failed,
            searched,
        }
    }

    /// Everything installed, across every available source, grouped.
    pub fn installed(&self) -> SearchResult {
        let sources = self.selected(&None);
        let searched: Vec<SourceKind> = sources.iter().map(|s| s.kind()).collect();
        let results: Vec<(SourceKind, Result<Vec<Package>>)> = std::thread::scope(|scope| {
            let handles: Vec<_> = sources
                .iter()
                .map(|s| {
                    let kind = s.kind();
                    scope.spawn(move || (kind, s.installed()))
                })
                .collect();
            handles.into_iter().map(|h| h.join().expect("a source panicked")).collect()
        });
        let mut packages = Vec::new();
        let mut failed = Vec::new();
        for (kind, result) in results {
            match result {
                Ok(mut found) => packages.append(&mut found),
                Err(e) => failed.push((kind, e.message)),
            }
        }
        let apps = group::group(packages, "");
        SearchResult {
            apps,
            failed,
            searched,
        }
    }

    /// Every update, across every available source.
    pub fn updates(&self) -> updates::UpdateList {
        updates::collect(self)
    }

    /// Build a plan for a list of operations, grouped so every root step of
    /// one source runs in one helper invocation.
    pub fn plan(&self, ops: &[Op]) -> Result<Plan> {
        transaction::plan::build(self, ops)
    }
}
