//! AppStream catalogue: the parser, the index and icon resolution.
//!
//! TODO: the real parser. This stub fixes the API the sources build against
//! and answers "nothing" to every question.
//!
//! A catalogue is a set of `<components>` XML files (gzip or plain) under a
//! directory such as `/usr/share/swcatalog/xml`, with cached icons beside it
//! under `icons/<origin>/<size>/`. The distribution's catalogue keys
//! components by `pkgname`; Flatpak's per-remote catalogue at
//! `/var/lib/flatpak/appstream/<remote>/<arch>/active/appstream.xml.gz`
//! keys them by `<bundle type="flatpak">app/<id>/<arch>/<branch></bundle>`.
//! One parser reads both.

use crate::model::{Picture, Screenshot, SystemInfo};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// A component from a catalogue, reduced to what the store draws from it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Component {
    /// The component id, with a trailing `.desktop` stripped so the Arch
    /// catalogue's `com.valvesoftware.Steam.desktop` and Flathub's
    /// `com.valvesoftware.Steam` are the same key.
    pub id: String,
    /// Which catalogue it came from: "archlinux-arch-extra", "flathub",
    /// "debian-bookworm-main". The origin attribute of the file.
    pub origin: String,
    /// The distribution package that provides it, where the catalogue says.
    pub pkgname: Option<String>,
    /// The Flatpak ref, where the catalogue is a remote's:
    /// `app/org.gimp.GIMP/x86_64/stable`.
    pub bundle: Option<String>,
    pub name: String,
    pub summary: Option<String>,
    /// The `<description>` markup, inner XML kept as the page expects it.
    pub description: Option<String>,
    pub developer: Option<String>,
    pub licence: Option<String>,
    pub homepage: Option<String>,
    pub categories: Vec<String>,
    pub keywords: Vec<String>,
    /// The largest cached icon that exists on disk, else a remote icon URL.
    pub icon: Option<Picture>,
    pub screenshots: Vec<Screenshot>,
    /// `type="desktop-application"` (or console-application, web-application).
    pub is_app: bool,
    /// Newest `<release>` version and its timestamp, where present.
    pub latest_release: Option<(String, Option<i64>)>,
}

/// Every component this machine knows about, indexed three ways.
#[derive(Debug, Default)]
pub struct Catalogue {
    components: Vec<Component>,
}

impl Catalogue {
    /// The distribution's catalogues: every XML under the standard
    /// directories, with icons resolved against their `icons/` siblings.
    /// Missing directories are simply absent; a file that will not parse is
    /// logged and skipped, never fatal.
    pub fn load_system(_system: &SystemInfo) -> Arc<Catalogue> {
        Arc::new(Catalogue::default())
    }

    /// Flatpak's catalogues for every remote in both installations
    /// (system and `~/.local/share/flatpak`), each tagged with the remote
    /// name as its origin.
    pub fn load_flatpak() -> Catalogue {
        Catalogue::default()
    }

    /// Read one directory of `*.xml` / `*.xml.gz` files, resolving cached
    /// icons against `icons_dir`, tagging every component with `origin` when
    /// the file does not state one.
    pub fn load_dir(
        _xml_dir: &Path,
        _icons_dir: Option<&Path>,
        _origin: Option<&str>,
    ) -> Catalogue {
        Catalogue::default()
    }

    /// Parse one catalogue file's bytes (gzip or plain XML).
    pub fn parse(
        _bytes: &[u8],
        _icons_dir: Option<&Path>,
        _origin: Option<&str>,
    ) -> crate::Result<Vec<Component>> {
        Ok(Vec::new())
    }

    pub fn from_components(components: Vec<Component>) -> Catalogue {
        Catalogue { components }
    }

    pub fn by_id(&self, _id: &str) -> Option<&Component> {
        None
    }

    pub fn by_pkgname(&self, _pkgname: &str) -> Option<&Component> {
        None
    }

    /// By Flatpak ref, `app/<id>/<arch>/<branch>`, or by just the id part.
    pub fn by_bundle(&self, _bundle: &str) -> Option<&Component> {
        None
    }

    /// Components matching the query in name, id, summary or keywords, best
    /// first. Used by the Flatpak source and as a fallback for any source
    /// whose own search is poor.
    pub fn search(&self, _query: &str, _limit: usize) -> Vec<&Component> {
        Vec::new()
    }

    pub fn components(&self) -> &[Component] {
        &self.components
    }

    pub fn is_empty(&self) -> bool {
        self.components.is_empty()
    }

    /// The standard XML directories, in the order they are read.
    pub fn system_xml_dirs() -> Vec<PathBuf> {
        [
            "/usr/share/swcatalog/xml",
            "/var/lib/swcatalog/xml",
            "/usr/share/app-info/xmls",
            "/var/lib/app-info/xmls",
        ]
        .iter()
        .map(PathBuf::from)
        .collect()
    }
}
