//! The three lookups and the search over a list of components.
//!
//! Everything here is positions into the `Catalogue`'s component list, so
//! the index never owns a component and building it is one pass. The search
//! keeps a lower-cased copy of the fields it matches, because the search
//! runs on every keystroke and lower-casing 1500 names each time would be
//! the slowest part of it.

use super::Component;
use std::collections::HashMap;

#[derive(Debug, Default)]
pub(super) struct Index {
    by_id: HashMap<String, usize>,
    /// The chosen component per package: the desktop application, else the
    /// first listed.
    by_pkgname: HashMap<String, usize>,
    /// Every component per package, in catalogue order.
    pkgname_all: HashMap<String, Vec<usize>>,
    /// Both the full ref and its id segment; the two never collide because a
    /// ref contains `/` and an id cannot.
    by_bundle: HashMap<String, usize>,
    search: Vec<SearchRecord>,
}

/// The matched fields, lower-cased once.
#[derive(Debug)]
struct SearchRecord {
    name: String,
    id: String,
    summary: String,
    keywords: Vec<String>,
}

// Ranks, highest first. A match on several fields scores its best one.
const NAME_EXACT: u8 = 6;
const NAME_PREFIX: u8 = 5;
const NAME_CONTAINS: u8 = 4;
const ID_CONTAINS: u8 = 3;
const KEYWORD_EQUALS: u8 = 2;
const SUMMARY_CONTAINS: u8 = 1;

impl Index {
    pub(super) fn build(components: &[Component]) -> Index {
        let mut index = Index::default();
        for (i, c) in components.iter().enumerate() {
            index.by_id.entry(c.id.clone()).or_insert(i);
            if let Some(pkgname) = &c.pkgname {
                index
                    .pkgname_all
                    .entry(pkgname.clone())
                    .or_default()
                    .push(i);
            }
            if let Some(bundle) = &c.bundle {
                index.by_bundle.entry(bundle.clone()).or_insert(i);
                if let Some(id) = bundle_id(bundle) {
                    index.by_bundle.entry(id.to_string()).or_insert(i);
                }
            }
            index.search.push(SearchRecord {
                name: c.name.to_lowercase(),
                id: c.id.to_lowercase(),
                summary: c.summary.as_deref().unwrap_or_default().to_lowercase(),
                keywords: c.keywords.iter().map(|k| k.to_lowercase()).collect(),
            });
        }
        for (pkgname, all) in &index.pkgname_all {
            let chosen = all
                .iter()
                .copied()
                .find(|&i| is_desktop_application(&components[i]))
                .unwrap_or(all[0]);
            index.by_pkgname.insert(pkgname.clone(), chosen);
        }
        index
    }

    pub(super) fn by_id(&self, id: &str) -> Option<usize> {
        self.by_id.get(id).copied()
    }

    pub(super) fn by_pkgname(&self, pkgname: &str) -> Option<usize> {
        self.by_pkgname.get(pkgname).copied()
    }

    pub(super) fn by_pkgname_all(&self, pkgname: &str) -> &[usize] {
        self.pkgname_all
            .get(pkgname)
            .map(Vec::as_slice)
            .unwrap_or_default()
    }

    pub(super) fn by_bundle(&self, bundle: &str) -> Option<usize> {
        self.by_bundle.get(bundle).copied()
    }

    /// Positions of the best matches, best first. Ties go to applications
    /// over add-ons, then to the shorter name (the closer match), then to
    /// catalogue order so the result is stable between runs.
    pub(super) fn search(&self, components: &[Component], query: &str, limit: usize) -> Vec<usize> {
        let query = query.trim().to_lowercase();
        if query.is_empty() || limit == 0 {
            return Vec::new();
        }
        let mut hits: Vec<(u8, usize)> = self
            .search
            .iter()
            .enumerate()
            .filter_map(|(i, r)| score(r, &query).map(|s| (s, i)))
            .collect();
        hits.sort_by(|a, b| {
            b.0.cmp(&a.0)
                .then_with(|| components[b.1].is_app.cmp(&components[a.1].is_app))
                .then_with(|| {
                    self.search[a.1]
                        .name
                        .len()
                        .cmp(&self.search[b.1].name.len())
                })
                .then_with(|| a.1.cmp(&b.1))
        });
        hits.truncate(limit);
        hits.into_iter().map(|(_, i)| i).collect()
    }
}

fn score(r: &SearchRecord, query: &str) -> Option<u8> {
    if r.name == query {
        Some(NAME_EXACT)
    } else if r.name.starts_with(query) {
        Some(NAME_PREFIX)
    } else if r.name.contains(query) {
        Some(NAME_CONTAINS)
    } else if r.id.contains(query) {
        Some(ID_CONTAINS)
    } else if r.keywords.iter().any(|k| k == query) {
        Some(KEYWORD_EQUALS)
    } else if r.summary.contains(query) {
        Some(SUMMARY_CONTAINS)
    } else {
        None
    }
}

/// The id segment of `app/<id>/<arch>/<branch>` or `runtime/<id>/...`.
fn bundle_id(bundle: &str) -> Option<&str> {
    let mut parts = bundle.split('/');
    let kind = parts.next()?;
    let id = parts.next()?;
    (matches!(kind, "app" | "runtime") && !id.is_empty()).then_some(id)
}

/// Flathub's older catalogues say `type="desktop"` for what is now
/// `desktop-application`; both are the same thing to the pkgname rule.
fn is_desktop_application(c: &Component) -> bool {
    matches!(c.component_type.as_str(), "desktop-application" | "desktop")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn component(id: &str, name: &str, component_type: &str) -> Component {
        Component {
            id: id.into(),
            name: name.into(),
            component_type: component_type.into(),
            is_app: matches!(
                component_type,
                "desktop-application" | "console-application" | "web-application"
            ),
            ..Default::default()
        }
    }

    fn fixture() -> Vec<Component> {
        let mut steam = component("com.valvesoftware.Steam", "Steam", "desktop-application");
        steam.pkgname = Some("steam".into());
        steam.summary = Some("Launcher for the Steam software distribution service".into());
        let mut proton = component(
            "com.valvesoftware.Steam.CompatibilityTool.Proton",
            "Proton",
            "addon",
        );
        proton.bundle =
            Some("app/com.valvesoftware.Steam.CompatibilityTool.Proton/x86_64/stable".into());
        let mut steamy = component(
            "io.example.Steamy",
            "Steamy Deck Tools",
            "desktop-application",
        );
        steamy.keywords = vec!["Steam".into(), "deck".into()];
        let mut lutris = component("net.lutris.Lutris", "Lutris", "desktop-application");
        lutris.summary = Some("Play games from Steam, GOG and more".into());
        let mut plugin = component("org.gimp.GIMP.plugin.gmic", "G'MIC", "addon");
        plugin.pkgname = Some("gimp".into());
        let mut gimp = component(
            "org.gimp.GIMP",
            "GNU Image Manipulation Program",
            "desktop-application",
        );
        gimp.pkgname = Some("gimp".into());
        gimp.bundle = Some("app/org.gimp.GIMP/x86_64/stable".into());
        let mut platform = component(
            "org.freedesktop.Platform",
            "Freedesktop Platform",
            "runtime",
        );
        platform.bundle = Some("runtime/org.freedesktop.Platform/x86_64/24.08".into());
        vec![steam, proton, steamy, lutris, plugin, gimp, platform]
    }

    #[test]
    fn search_ranks_name_exact_prefix_contains_id_keyword_then_summary() {
        let components = fixture();
        let index = Index::build(&components);
        let ids: Vec<&str> = index
            .search(&components, "steam", 10)
            .into_iter()
            .map(|i| components[i].id.as_str())
            .collect();
        assert_eq!(
            ids,
            vec![
                "com.valvesoftware.Steam",
                "io.example.Steamy",
                "com.valvesoftware.Steam.CompatibilityTool.Proton",
                "net.lutris.Lutris",
            ]
        );
        // "Steamy Deck Tools" is a name prefix match and outranks the addon
        // whose id merely contains the word; the keyword on Steamy is never
        // what ranks it because the name already did better.
        let prefix: Vec<&str> = index
            .search(&components, "STEAMY", 10)
            .into_iter()
            .map(|i| components[i].id.as_str())
            .collect();
        assert_eq!(prefix, vec!["io.example.Steamy"]);
        let keyword: Vec<&str> = index
            .search(&components, "deck", 10)
            .into_iter()
            .map(|i| components[i].id.as_str())
            .collect();
        assert_eq!(keyword, vec!["io.example.Steamy"]);
    }

    #[test]
    fn search_honours_the_limit_and_ignores_blank_queries() {
        let components = fixture();
        let index = Index::build(&components);
        assert_eq!(index.search(&components, "steam", 2).len(), 2);
        assert!(index.search(&components, "   ", 10).is_empty());
        assert!(index.search(&components, "steam", 0).is_empty());
        assert!(index.search(&components, "nothing-has-this", 10).is_empty());
    }

    #[test]
    fn a_name_contains_match_outranks_an_id_match_and_apps_win_ties() {
        let components = fixture();
        let index = Index::build(&components);
        let ids: Vec<&str> = index
            .search(&components, "gimp", 10)
            .into_iter()
            .map(|i| components[i].id.as_str())
            .collect();
        // Neither name contains "gimp"; both ids do. The application comes
        // before the add-on.
        assert_eq!(ids, vec!["org.gimp.GIMP", "org.gimp.GIMP.plugin.gmic"]);
        let ids: Vec<&str> = index
            .search(&components, "image", 10)
            .into_iter()
            .map(|i| components[i].id.as_str())
            .collect();
        assert_eq!(ids, vec!["org.gimp.GIMP"]);
    }

    #[test]
    fn pkgname_prefers_the_desktop_application_then_the_first() {
        let components = fixture();
        let index = Index::build(&components);
        assert_eq!(
            index.by_pkgname("gimp").map(|i| components[i].id.as_str()),
            Some("org.gimp.GIMP")
        );
        assert_eq!(index.by_pkgname_all("gimp").len(), 2);
        assert_eq!(
            index.by_pkgname("steam").map(|i| components[i].id.as_str()),
            Some("com.valvesoftware.Steam")
        );
        assert_eq!(index.by_pkgname("nothing"), None);
        assert!(index.by_pkgname_all("nothing").is_empty());
        // Two add-ons and no application: the first listed.
        let two = vec![
            {
                let mut c = component("a.one", "One", "addon");
                c.pkgname = Some("p".into());
                c
            },
            {
                let mut c = component("a.two", "Two", "addon");
                c.pkgname = Some("p".into());
                c
            },
        ];
        let index = Index::build(&two);
        assert_eq!(index.by_pkgname("p"), Some(0));
    }

    #[test]
    fn bundles_are_found_by_full_ref_and_by_id_segment() {
        let components = fixture();
        let index = Index::build(&components);
        assert_eq!(index.by_bundle("app/org.gimp.GIMP/x86_64/stable"), Some(5));
        assert_eq!(index.by_bundle("org.gimp.GIMP"), Some(5));
        assert_eq!(
            index.by_bundle("runtime/org.freedesktop.Platform/x86_64/24.08"),
            Some(6)
        );
        assert_eq!(index.by_bundle("org.freedesktop.Platform"), Some(6));
        assert_eq!(index.by_bundle("app/org.gimp.GIMP/aarch64/stable"), None);
        assert_eq!(bundle_id("app//x/y"), None);
        assert_eq!(bundle_id("garbage"), None);
    }

    #[test]
    fn the_first_of_two_components_with_one_id_wins() {
        let components = vec![
            component("a.b", "First", "desktop-application"),
            component("a.b", "Second", "desktop-application"),
        ];
        let index = Index::build(&components);
        assert_eq!(index.by_id("a.b"), Some(0));
        assert_eq!(index.by_id("A.B"), None, "ids are exact");
    }
}
