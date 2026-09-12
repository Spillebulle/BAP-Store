//! Every source's updates, merged into the one list the Updates page draws.
//!
//! [`collect`] asks every available source at once and keeps every failure
//! with its sentence, so a source that could not answer is a line on the
//! page and never a silent gap. The list is ordered for reading: BAP Store's
//! own update first, then applications, then packages, each by name.
//!
//! This module holds no cache. The ten-minute cache the Settings page
//! promises is the application's, kept beside the window where "last
//! checked" is drawn; a cache here would make the text mode's
//! `bap-store updates` answer stale on its second run.

use crate::model::*;
use crate::{Result, Source, Store};
use std::collections::HashSet;

/// The application id, for recognising BAP Store among Flatpak refs.
const APP_ID: &str = "io.github.spillebulle.bapstore";

#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct UpdateList {
    pub updates: Vec<Update>,
    pub failed: Vec<(SourceKind, String)>,
    /// Unix seconds when this list was made.
    pub checked_at: i64,
}

impl UpdateList {
    /// The bytes to fetch across every update whose source knows its size.
    /// Use with [`UpdateList::without_size`]: when that is not zero the page says
    /// "at least", because a total that quietly skips unknowns is a
    /// fraction drawn over a guess.
    pub fn total_download(&self) -> u64 {
        self.updates.iter().filter_map(|u| u.download_size).sum()
    }

    /// How many updates have no known size.
    pub fn without_size(&self) -> usize {
        self.updates
            .iter()
            .filter(|u| u.download_size.is_none())
            .count()
    }

    /// How many updates each source has, in interface order, leaving out
    /// sources with none so the summary reads "3 from pacman, 1 from Flatpak"
    /// rather than a row of zeros.
    pub fn by_source(&self) -> Vec<(SourceKind, usize)> {
        SourceKind::ALL
            .into_iter()
            .map(|kind| {
                (
                    kind,
                    self.updates
                        .iter()
                        .filter(|u| u.package.source == kind)
                        .count(),
                )
            })
            .filter(|(_, n)| *n > 0)
            .collect()
    }

    /// BAP Store's own update, when one is in the list.
    pub fn self_update(&self) -> Option<&Update> {
        self.updates.iter().find(|u| u.is_self)
    }
}

/// Ask every available source at once and merge what comes back.
pub fn collect(store: &Store) -> UpdateList {
    let sources: Vec<&dyn Source> = store
        .sources
        .iter()
        .map(|s| s.as_ref())
        .filter(|s| s.status().available)
        .collect();
    let results: Vec<(SourceKind, Result<Vec<Update>>)> = std::thread::scope(|scope| {
        let handles: Vec<_> = sources
            .iter()
            .map(|s| {
                let kind = s.kind();
                scope.spawn(move || (kind, s.updates()))
            })
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().expect("a source panicked"))
            .collect()
    });
    merge(results)
}

/// The pure half of [`collect`]: record failures, drop a duplicate
/// `(source, id)`, mark BAP Store's own row, sort.
pub fn merge(results: Vec<(SourceKind, Result<Vec<Update>>)>) -> UpdateList {
    let mut list = UpdateList::default();
    let mut seen: HashSet<(SourceKind, String)> = HashSet::new();
    for (kind, result) in results {
        match result {
            Ok(found) => {
                for mut update in found {
                    if !seen.insert((update.package.source, update.package.id.clone())) {
                        continue;
                    }
                    // A source is asked to set `is_self`; this is the net
                    // under it, so the store's own row is first whichever
                    // source found it.
                    update.is_self = update.is_self || is_self_package(&update.package);
                    list.updates.push(update);
                }
            }
            Err(e) => list.failed.push((kind, e.message)),
        }
    }
    // Stable, so two rows with the same name keep source order.
    list.updates.sort_by_cached_key(|u| {
        (
            !u.is_self,
            u.kind != PackageKind::App,
            u.name.to_lowercase(),
        )
    });
    list.failed.sort_by_key(|(kind, _)| *kind);
    list.checked_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    list
}

/// Whether a package reference is BAP Store itself, by the names each
/// source would use for it.
fn is_self_package(package: &PackageRef) -> bool {
    match package.source {
        SourceKind::Pacman
        | SourceKind::Aur
        | SourceKind::Apt
        | SourceKind::Dnf
        | SourceKind::Snap => {
            matches!(package.id.as_str(), "bap-store" | "bap-store-bin")
        }
        SourceKind::Flatpak => package.id.contains(APP_ID),
        SourceKind::Github => package.id.eq_ignore_ascii_case("Spillebulle/BAP-Store"),
        SourceKind::Fwupd | SourceKind::Chwd => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Error, Op, Query};

    /// A source that answers from a table.
    struct Fake {
        kind: SourceKind,
        available: bool,
        updates: Vec<Update>,
        fail: Option<String>,
    }

    impl Source for Fake {
        fn kind(&self) -> SourceKind {
            self.kind
        }
        fn status(&self) -> SourceStatus {
            SourceStatus {
                kind: self.kind,
                available: self.available,
                reason: (!self.available).then(|| "Not on this machine.".to_string()),
                detail: None,
            }
        }
        fn search(&self, _query: &Query) -> Result<Vec<Package>> {
            Ok(Vec::new())
        }
        fn installed(&self) -> Result<Vec<Package>> {
            Ok(Vec::new())
        }
        fn updates(&self) -> Result<Vec<Update>> {
            match &self.fail {
                Some(message) => Err(Error::from_source(self.kind, message.clone())),
                None => Ok(self.updates.clone()),
            }
        }
        fn details(&self, id: &str) -> Result<Package> {
            Err(Error::from_source(
                self.kind,
                format!("{id} is not known to this source."),
            ))
        }
        fn plan(&self, _op: &Op) -> Result<Vec<Step>> {
            Ok(Vec::new())
        }
    }

    fn update(
        source: SourceKind,
        id: &str,
        name: &str,
        kind: PackageKind,
        size: Option<u64>,
    ) -> Update {
        Update {
            package: PackageRef {
                source,
                id: id.to_string(),
            },
            name: name.to_string(),
            kind,
            summary: None,
            icon: None,
            from: Some("1.0".to_string()),
            to: "1.1".to_string(),
            download_size: size,
            published: None,
            is_self: false,
        }
    }

    fn store(sources: Vec<Fake>) -> Store {
        Store {
            system: crate::system::from_os_release(""),
            sources: sources
                .into_iter()
                .map(|s| Box::new(s) as Box<dyn Source>)
                .collect(),
        }
    }

    #[test]
    fn every_available_source_is_asked_and_a_failure_is_a_sentence_not_a_gap() {
        let store = store(vec![
            Fake {
                kind: SourceKind::Pacman,
                available: true,
                updates: vec![update(
                    SourceKind::Pacman,
                    "zlib",
                    "zlib",
                    PackageKind::Package,
                    Some(100),
                )],
                fail: None,
            },
            Fake {
                kind: SourceKind::Aur,
                available: true,
                updates: Vec::new(),
                fail: Some("aur.archlinux.org did not answer in time".to_string()),
            },
            Fake {
                kind: SourceKind::Flatpak,
                available: false,
                updates: vec![update(
                    SourceKind::Flatpak,
                    "x",
                    "Should not appear",
                    PackageKind::App,
                    None,
                )],
                fail: None,
            },
        ]);
        let list = collect(&store);
        assert_eq!(list.updates.len(), 1);
        assert_eq!(list.updates[0].name, "zlib");
        assert_eq!(
            list.failed,
            vec![(
                SourceKind::Aur,
                "aur.archlinux.org did not answer in time".to_string()
            )]
        );
        assert!(list.checked_at > 0);
    }

    #[test]
    fn the_store_itself_comes_first_then_apps_then_packages_by_name() {
        let results = vec![
            (
                SourceKind::Pacman,
                Ok(vec![
                    update(
                        SourceKind::Pacman,
                        "zlib",
                        "zlib",
                        PackageKind::Package,
                        None,
                    ),
                    update(
                        SourceKind::Pacman,
                        "Alpha-lib",
                        "Alpha-lib",
                        PackageKind::Package,
                        None,
                    ),
                    update(SourceKind::Pacman, "steam", "Steam", PackageKind::App, None),
                ]),
            ),
            (
                SourceKind::Flatpak,
                Ok(vec![
                    update(
                        SourceKind::Flatpak,
                        "org.gimp.GIMP",
                        "GIMP",
                        PackageKind::App,
                        None,
                    ),
                    update(
                        SourceKind::Flatpak,
                        "org.example.Runtime",
                        "Freedesktop runtime",
                        PackageKind::Runtime,
                        None,
                    ),
                ]),
            ),
            (
                SourceKind::Aur,
                Ok(vec![update(
                    SourceKind::Aur,
                    "bap-store",
                    "BAP Store",
                    PackageKind::App,
                    None,
                )]),
            ),
        ];
        let names: Vec<String> = merge(results).updates.into_iter().map(|u| u.name).collect();
        assert_eq!(
            names,
            [
                "BAP Store",
                "GIMP",
                "Steam",
                "Alpha-lib",
                "Freedesktop runtime",
                "zlib"
            ]
        );
    }

    #[test]
    fn a_source_that_forgets_is_self_is_corrected() {
        for (source, id) in [
            (SourceKind::Pacman, "bap-store"),
            (SourceKind::Aur, "bap-store-bin"),
            (SourceKind::Apt, "bap-store"),
            (SourceKind::Dnf, "bap-store"),
            (
                SourceKind::Flatpak,
                "flathub/app/io.github.spillebulle.bapstore/x86_64/stable",
            ),
            (SourceKind::Github, "spillebulle/bap-store"),
        ] {
            let list = merge(vec![(
                source,
                Ok(vec![update(source, id, "x", PackageKind::App, None)]),
            )]);
            assert!(list.updates[0].is_self, "{source:?} {id}");
            assert!(list.self_update().is_some());
        }
        let list = merge(vec![(
            SourceKind::Pacman,
            Ok(vec![update(
                SourceKind::Pacman,
                "bap",
                "bap",
                PackageKind::Package,
                None,
            )]),
        )]);
        assert!(!list.updates[0].is_self);
        assert!(list.self_update().is_none());
    }

    #[test]
    fn an_identical_source_and_id_is_listed_once() {
        let list = merge(vec![
            (
                SourceKind::Pacman,
                Ok(vec![
                    update(
                        SourceKind::Pacman,
                        "zlib",
                        "zlib",
                        PackageKind::Package,
                        Some(10),
                    ),
                    update(
                        SourceKind::Pacman,
                        "zlib",
                        "zlib",
                        PackageKind::Package,
                        Some(10),
                    ),
                ]),
            ),
            // The same id from another source is a different thing.
            (
                SourceKind::Aur,
                Ok(vec![update(
                    SourceKind::Aur,
                    "zlib",
                    "zlib",
                    PackageKind::Package,
                    Some(5),
                )]),
            ),
        ]);
        assert_eq!(list.updates.len(), 2);
        assert_eq!(list.total_download(), 15);
    }

    #[test]
    fn totals_and_counts_are_honest_about_what_is_unknown() {
        let list = merge(vec![
            (
                SourceKind::Pacman,
                Ok(vec![
                    update(
                        SourceKind::Pacman,
                        "a",
                        "a",
                        PackageKind::Package,
                        Some(100),
                    ),
                    update(SourceKind::Pacman, "b", "b", PackageKind::Package, None),
                ]),
            ),
            (
                SourceKind::Flatpak,
                Ok(vec![update(
                    SourceKind::Flatpak,
                    "c",
                    "c",
                    PackageKind::App,
                    Some(250),
                )]),
            ),
            (SourceKind::Snap, Ok(Vec::new())),
        ]);
        assert_eq!(list.total_download(), 350);
        assert_eq!(list.without_size(), 1);
        assert_eq!(
            list.by_source(),
            vec![(SourceKind::Pacman, 2), (SourceKind::Flatpak, 1)]
        );
        assert_eq!(UpdateList::default().by_source(), Vec::new());
        assert_eq!(UpdateList::default().total_download(), 0);
    }

    #[test]
    fn failures_are_listed_in_interface_order_whatever_order_the_threads_finished() {
        let list = merge(vec![
            (SourceKind::Snap, Err(Error::new("snapd is not running."))),
            (
                SourceKind::Pacman,
                Err(Error::new("The database is locked.")),
            ),
        ]);
        assert_eq!(list.failed[0].0, SourceKind::Pacman);
        assert_eq!(list.failed[1].0, SourceKind::Snap);
    }
}
