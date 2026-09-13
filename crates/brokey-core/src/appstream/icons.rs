//! Cached icons: from the names a component declares to a file that exists.
//!
//! A catalogue declares `<icon type="cached" width="64">steam_steam.png</icon>`
//! and the file sits in a size directory: `icons/<origin>/64x64/` for a
//! distribution's catalogue, `icons/64x64/` for a Flatpak remote's. The
//! declared sizes are what the generator meant to write, not what is on
//! disk (the Arch `core` catalogue declares a 64 px icon for `links` and
//! ships it in `64x64/` only, while `extra` declares three sizes and is
//! missing the 128 px file for a few dozen), so the answer comes from
//! `stat`, never from the declaration.

use crate::model::Picture;
use std::path::{Component as PathComponent, Path, PathBuf};

/// Largest first: the page's hero is 96 px and the row 32 px, so the
/// largest file is the one every use scales down from.
const SIZES: [&str; 3] = ["128x128", "64x64", "48x48"];

/// The directory the size directories live in, decided once per file.
#[derive(Clone, Debug)]
pub(super) struct IconDir {
    dir: PathBuf,
}

impl IconDir {
    /// Distribution catalogues nest the origin under `icons/`; Flatpak's
    /// per-remote directory does not. Whether `<icons_dir>/<origin>` exists
    /// tells them apart, so one parser serves both without being told.
    pub(super) fn new(icons_dir: Option<&Path>, origin: &str) -> Option<IconDir> {
        let icons_dir = icons_dir?;
        let by_origin = icons_dir.join(origin);
        let dir = if !origin.is_empty() && by_origin.is_dir() {
            by_origin
        } else {
            icons_dir.to_path_buf()
        };
        Some(IconDir { dir })
    }

    /// The largest size on disk among the declared cached icon names.
    /// `cached` is `(declared width, file name)`; names are tried in order
    /// of declared width so a component that declares different files per
    /// size still gets its biggest.
    pub(super) fn resolve(&self, cached: &[(u32, String)]) -> Option<PathBuf> {
        let mut names: Vec<&(u32, String)> = cached
            .iter()
            .filter(|(_, n)| is_plain_file_name(n))
            .collect();
        names.sort_by_key(|a| std::cmp::Reverse(a.0));
        names.dedup_by(|a, b| a.1 == b.1);
        for size in SIZES {
            for (_, name) in &names {
                let path = self.dir.join(size).join(name);
                if path.is_file() {
                    return Some(path);
                }
            }
        }
        None
    }
}

/// The picture for a component: the largest cached icon on disk, else the
/// remote URL, else nothing. A path that does not exist is never returned,
/// so the page never draws a broken image where a `control` block belongs.
pub(super) fn pick(
    cached: &[(u32, String)],
    remote: Option<&str>,
    dir: Option<&IconDir>,
) -> Option<Picture> {
    if let Some(path) = dir.and_then(|d| d.resolve(cached)) {
        return Some(Picture::File(path));
    }
    remote.map(|url| Picture::Url(url.to_string()))
}

/// A catalogue is data from the network; an icon name with a directory in
/// it would make the store stat, and then serve, a file the catalogue
/// chose. Only a bare file name is looked up.
fn is_plain_file_name(name: &str) -> bool {
    let mut parts = Path::new(name).components();
    matches!(
        (parts.next(), parts.next()),
        (Some(PathComponent::Normal(_)), None)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn touch(path: &Path) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, b"png").unwrap();
    }

    #[test]
    fn the_origin_directory_is_used_when_it_exists_and_flat_otherwise() {
        let tmp = tempfile::tempdir().unwrap();
        let icons = tmp.path().join("icons");
        touch(&icons.join("archlinux-arch-extra/64x64/a.png"));
        touch(&icons.join("64x64/b.png"));
        let by_origin = IconDir::new(Some(&icons), "archlinux-arch-extra").unwrap();
        assert_eq!(by_origin.dir, icons.join("archlinux-arch-extra"));
        let flat = IconDir::new(Some(&icons), "flathub").unwrap();
        assert_eq!(flat.dir, icons);
        assert!(IconDir::new(None, "flathub").is_none());
    }

    #[test]
    fn the_largest_existing_size_wins_and_a_missing_file_is_none() {
        let tmp = tempfile::tempdir().unwrap();
        let icons = tmp.path().join("icons");
        touch(&icons.join("48x48/x.png"));
        touch(&icons.join("64x64/x.png"));
        touch(&icons.join("48x48/y.png"));
        let dir = IconDir::new(Some(&icons), "flathub").unwrap();
        let declared = |n: &str| {
            vec![
                (48, n.to_string()),
                (64, n.to_string()),
                (128, n.to_string()),
            ]
        };
        assert_eq!(
            dir.resolve(&declared("x.png")),
            Some(icons.join("64x64/x.png"))
        );
        assert_eq!(
            dir.resolve(&declared("y.png")),
            Some(icons.join("48x48/y.png"))
        );
        assert_eq!(dir.resolve(&declared("z.png")), None);
        assert_eq!(dir.resolve(&[]), None);
    }

    #[test]
    fn different_names_per_size_are_tried_biggest_declared_first() {
        let tmp = tempfile::tempdir().unwrap();
        let icons = tmp.path().join("icons");
        touch(&icons.join("128x128/big.png"));
        touch(&icons.join("128x128/small.png"));
        let dir = IconDir::new(Some(&icons), "").unwrap();
        let declared = vec![(48, "small.png".to_string()), (128, "big.png".to_string())];
        assert_eq!(dir.resolve(&declared), Some(icons.join("128x128/big.png")));
    }

    #[test]
    fn names_with_directories_are_never_looked_up() {
        let tmp = tempfile::tempdir().unwrap();
        let icons = tmp.path().join("icons");
        touch(&icons.join("64x64/x.png"));
        let dir = IconDir::new(Some(&icons), "").unwrap();
        assert_eq!(dir.resolve(&[(64, "../64x64/x.png".to_string())]), None);
        assert_eq!(dir.resolve(&[(64, "/etc/passwd".to_string())]), None);
        assert!(!is_plain_file_name(""));
        assert!(is_plain_file_name("steam_steam.png"));
    }

    #[test]
    fn pick_falls_back_to_the_remote_url_then_to_nothing() {
        let tmp = tempfile::tempdir().unwrap();
        let icons = tmp.path().join("icons");
        touch(&icons.join("48x48/x.png"));
        let dir = IconDir::new(Some(&icons), "").unwrap();
        let cached = vec![(48, "x.png".to_string())];
        assert_eq!(
            pick(&cached, Some("https://e/x.png"), Some(&dir)),
            Some(Picture::File(icons.join("48x48/x.png")))
        );
        let missing = vec![(48, "gone.png".to_string())];
        assert_eq!(
            pick(&missing, Some("https://e/x.png"), Some(&dir)),
            Some(Picture::Url("https://e/x.png".into()))
        );
        assert_eq!(pick(&missing, None, Some(&dir)), None);
        assert_eq!(pick(&cached, None, None), None);
    }
}
