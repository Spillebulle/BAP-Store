//! The one true thing to say about getting a newer BAP Store, given how this
//! copy was installed (style guide §18.3).
//!
//! **A command in an interface is a claim that running it will do
//! something.** An upgrade command given to a machine with nowhere to
//! upgrade from is worse than no message, because the package manager
//! answers it confidently and the user concludes they are up to date. So
//! every remedy here is one of four shapes, chosen from the
//! [`Installation`] and never from a guess:
//!
//! * [`Remedy::UpdatesPage`]: the package manager already has it (the AUR,
//!   or the Spillebulle archive), so it is a row on the Updates page.
//! * [`Remedy::InstallAsset`]: the manager has never heard of it, so the
//!   exact release asset is downloaded and handed to the manager through
//!   the helper.
//! * [`Remedy::ReplaceFile`]: the copy is BAP Store's own file, so the new
//!   one is put in its place.
//! * [`Remedy::Sentence`]: nothing BAP Store can do; the sentence says what
//!   the user can. It names a file only when that file is in the release.
//!
//! Where the release carries no asset for this machine's architecture, the
//! remedy is a sentence and no file name, ever: inventing one sends somebody
//! looking for a file that has never existed. `tests/selfupdate.rs` holds
//! the guard that fails if any forbidden command reaches a sentence.

use super::install::{Installation, PACMAN_SOURCE_PACKAGE};
use super::release::Asset;
use super::version::Version;
use crate::model::SourceKind;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// The name the `.deb` and the `.rpm` are published under, which is the
/// name apt and dnf list the update by once the archive is enrolled.
pub const ARCHIVE_PACKAGE: &str = "bap-store";

/// The architectures the release workflow builds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Arch {
    X86_64,
    Aarch64,
}

impl Arch {
    /// From `std::env::consts::ARCH` or a package manager's own spelling.
    /// `None` for an architecture no release is built for; a source build
    /// on riscv64 is a real case and gets a sentence with no file name.
    pub fn parse(text: &str) -> Option<Arch> {
        match text {
            "x86_64" | "amd64" | "x64" => Some(Arch::X86_64),
            "aarch64" | "arm64" => Some(Arch::Aarch64),
            _ => None,
        }
    }

    /// Debian's spelling, the one in a `.deb` file's name.
    pub fn deb(self) -> &'static str {
        match self {
            Arch::X86_64 => "amd64",
            Arch::Aarch64 => "arm64",
        }
    }

    /// rpm's spelling, which the pacman package, the AppImage and the
    /// Flatpak bundle use too.
    pub fn rpm(self) -> &'static str {
        match self {
            Arch::X86_64 => "x86_64",
            Arch::Aarch64 => "aarch64",
        }
    }
}

/// The shapes a release publishes. [`Format::name`] is the exact file name
/// the release workflow must produce; a name there that changes is the
/// other half of a change here, and `tests/selfupdate.rs` pins both.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    /// `bap-store_<v>_amd64.deb`, `bap-store_<v>_arm64.deb`
    Deb,
    /// `bap-store-<v>-1.x86_64.rpm`, `bap-store-<v>-1.aarch64.rpm`
    Rpm,
    /// `bap-store-bin-<v>-1-x86_64.pkg.tar.zst`
    PacmanPackage,
    /// `BAP-Store-<v>-x86_64.AppImage`, `BAP-Store-<v>-aarch64.AppImage`
    AppImage,
    /// `bap-store-<v>-x86_64.flatpak`
    Flatpak,
}

impl Format {
    /// The file name the workflow gives this format at this version.
    pub fn name(self, version: &Version, arch: Arch) -> String {
        match self {
            Format::Deb => format!("bap-store_{version}_{}.deb", arch.deb()),
            Format::Rpm => format!("bap-store-{version}-1.{}.rpm", arch.rpm()),
            Format::PacmanPackage => {
                format!("bap-store-bin-{version}-1-{}.pkg.tar.zst", arch.rpm())
            }
            Format::AppImage => format!("BAP-Store-{version}-{}.AppImage", arch.rpm()),
            Format::Flatpak => format!("bap-store-{version}-{}.flatpak", arch.rpm()),
        }
    }

    /// Whether an asset name is this format for this architecture.
    ///
    /// Matched on the prefix and the suffix the workflow builds the name
    /// from, not on the whole name: the version sits in the middle, and the
    /// package release (`-1`) is the packager's to bump. Writing either out
    /// here would mean this code deciding what the next release is called.
    pub fn matches(self, arch: Arch, name: &str) -> bool {
        let (prefix, suffix) = match self {
            Format::Deb => ("bap-store_", format!("_{}.deb", arch.deb())),
            Format::Rpm => ("bap-store-", format!(".{}.rpm", arch.rpm())),
            Format::PacmanPackage => ("bap-store-bin-", format!("-{}.pkg.tar.zst", arch.rpm())),
            Format::AppImage => ("BAP-Store-", format!("-{}.AppImage", arch.rpm())),
            Format::Flatpak => ("bap-store-", format!("-{}.flatpak", arch.rpm())),
        };
        name.starts_with(prefix) && name.ends_with(&suffix)
    }

    /// What a sentence calls it.
    pub fn label(self) -> &'static str {
        match self {
            Format::Deb => "Debian package",
            Format::Rpm => "rpm package",
            Format::PacmanPackage => "pacman package",
            Format::AppImage => "AppImage",
            Format::Flatpak => "Flatpak bundle",
        }
    }
}

/// The asset of this format for this architecture, if the release carries
/// one on `https`. `None` for an architecture no release is built for.
pub fn find(assets: &[Asset], format: Format, arch: Option<Arch>) -> Option<&Asset> {
    let arch = arch?;
    assets
        .iter()
        .find(|a| a.is_fetchable() && format.matches(arch, &a.name))
}

/// How a downloaded asset is installed. Every one but the Flatpak bundle
/// runs as root through the helper.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Installer {
    /// `pacman -U <file>`
    PacmanU,
    /// `apt-get install -y <file>`, which resolves dependencies where a bare
    /// `dpkg -i` would leave them unmet.
    DpkgI,
    /// `dnf install -y <file>`, for the same reason over `rpm -U`.
    RpmU,
    /// `flatpak install --user -y <file>`. No remedy produces this today: a
    /// store inside the sandbox cannot run `flatpak`, so the Flatpak case is
    /// a sentence. Kept so the plan has an answer if the bundle ever runs
    /// unsandboxed.
    FlatpakBundle,
}

/// What to do, and the one sentence that says so.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Remedy {
    /// The manager already has it: it is a row on the Updates page.
    UpdatesPage {
        source: SourceKind,
        package: String,
        sentence: String,
    },
    /// Download the exact asset and hand it to the manager.
    InstallAsset {
        asset: String,
        url: String,
        installer: Installer,
        sentence: String,
    },
    /// Download the exact asset and put it in place of the file at `path`.
    ReplaceFile {
        path: PathBuf,
        asset: String,
        url: String,
        sentence: String,
    },
    /// Nothing BAP Store can run. A struct variant rather than a newtype,
    /// because an internally tagged enum cannot serialise a bare string and
    /// because every variant then carries `sentence` for the page to read.
    Sentence { sentence: String },
}

impl Remedy {
    pub fn sentence(&self) -> &str {
        match self {
            Remedy::UpdatesPage { sentence, .. }
            | Remedy::InstallAsset { sentence, .. }
            | Remedy::ReplaceFile { sentence, .. }
            | Remedy::Sentence { sentence } => sentence,
        }
    }

    /// The release asset this remedy would fetch, if any.
    pub fn asset(&self) -> Option<&str> {
        match self {
            Remedy::InstallAsset { asset, .. } | Remedy::ReplaceFile { asset, .. } => Some(asset),
            Remedy::UpdatesPage { .. } | Remedy::Sentence { .. } => None,
        }
    }

    /// Whether BAP Store can carry this out itself, as a plan.
    pub fn is_actionable(&self) -> bool {
        matches!(
            self,
            Remedy::InstallAsset { .. } | Remedy::ReplaceFile { .. }
        )
    }
}

/// The remedy for an installation, given the newest version and the assets
/// its release carries. `arch` is `std::env::consts::ARCH` or a manager's
/// spelling; anything unrecognised gets sentences with no file name.
pub fn remedy(
    installation: &Installation,
    latest: &Version,
    arch: &str,
    assets: &[Asset],
) -> Remedy {
    let arch_known = Arch::parse(arch);
    let asset = |format: Format| find(assets, format, arch_known);

    match installation {
        Installation::Pacman { package } if package == PACMAN_SOURCE_PACKAGE => {
            Remedy::UpdatesPage {
                source: SourceKind::Aur,
                package: package.clone(),
                sentence: format!("BAP Store {latest} is in the AUR; it is in your Updates."),
            }
        }

        Installation::Pacman { .. } => match asset(Format::PacmanPackage) {
            Some(a) => Remedy::InstallAsset {
                asset: a.name.clone(),
                url: a.browser_download_url.clone(),
                installer: Installer::PacmanU,
                sentence: format!(
                    "BAP Store {latest} is not in a repository this machine uses. \
                     BAP Store will download {} and install it with pacman.",
                    a.name
                ),
            },
            None => Remedy::Sentence {
                sentence: none_for(Format::PacmanPackage, latest, arch),
            },
        },

        Installation::Dpkg { archive: true } => Remedy::UpdatesPage {
            source: SourceKind::Apt,
            package: ARCHIVE_PACKAGE.to_string(),
            sentence: format!(
                "BAP Store {latest} is in the Spillebulle archive; it is in your Updates."
            ),
        },

        Installation::Dpkg { archive: false } => match asset(Format::Deb) {
            Some(a) => Remedy::InstallAsset {
                asset: a.name.clone(),
                url: a.browser_download_url.clone(),
                installer: Installer::DpkgI,
                sentence: format!(
                    "This machine does not have the Spillebulle archive, so apt has nothing newer to offer. \
                     BAP Store will download {} and install it with apt. That package adds the archive, \
                     and apt keeps BAP Store up to date from then on.",
                    a.name
                ),
            },
            None => Remedy::Sentence {
                sentence: none_for(Format::Deb, latest, arch),
            },
        },

        Installation::Rpm { archive: true } => Remedy::UpdatesPage {
            source: SourceKind::Dnf,
            package: ARCHIVE_PACKAGE.to_string(),
            sentence: format!(
                "BAP Store {latest} is in the Spillebulle archive; it is in your Updates."
            ),
        },

        Installation::Rpm { archive: false } => match asset(Format::Rpm) {
            Some(a) => Remedy::InstallAsset {
                asset: a.name.clone(),
                url: a.browser_download_url.clone(),
                installer: Installer::RpmU,
                sentence: format!(
                    "This machine does not have the Spillebulle archive, so dnf has nothing newer to offer. \
                     BAP Store will download {} and install it with dnf. That package adds the archive, \
                     and your usual system update keeps BAP Store up to date from then on.",
                    a.name
                ),
            },
            None => Remedy::Sentence {
                sentence: none_for(Format::Rpm, latest, arch),
            },
        },

        // A store inside the sandbox cannot run `flatpak`, so even with the
        // bundle in hand there is nothing to run: the sentence names the
        // file and the user installs it.
        Installation::Flatpak => Remedy::Sentence {
            sentence: match asset(Format::Flatpak) {
                Some(a) => format!(
                    "This copy runs as a Flatpak bundle with no remote behind it. \
                     Take {} from the releases page and install it over this one.",
                    a.name
                ),
                None => format!(
                    "This copy runs as a Flatpak bundle with no remote behind it, and BAP Store {latest} \
                     has no Flatpak bundle for {} on the releases page, so this copy stays as it is \
                     until one is published.",
                    arch_word(arch)
                ),
            },
        },

        Installation::AppImage { path } => match asset(Format::AppImage) {
            Some(a) => Remedy::ReplaceFile {
                path: path.clone(),
                asset: a.name.clone(),
                url: a.browser_download_url.clone(),
                sentence: format!(
                    "BAP Store will download {} and put it in place of {}.",
                    a.name,
                    path.display()
                ),
            },
            None => Remedy::Sentence {
                sentence: none_for(Format::AppImage, latest, arch),
            },
        },

        // No tarball is published, so a portable copy has nothing of its
        // own shape to be replaced with; the AppImage is the same
        // application in one file, and is named when it exists.
        Installation::Portable => Remedy::Sentence {
            sentence: match asset(Format::AppImage) {
                Some(a) => format!(
                    "BAP Store publishes no archive for a copy like this one. \
                     Take {} from the releases page and run it in place of this copy.",
                    a.name
                ),
                None => format!(
                    "BAP Store publishes no archive for a copy like this one, and {latest} has no AppImage \
                     for {} on the releases page, so this copy stays as it is until one is published.",
                    arch_word(arch)
                ),
            },
        },

        Installation::Unknown => Remedy::Sentence {
            sentence: "This copy was installed in a way BAP Store cannot update by itself. \
                       Update it the way it was installed."
                .to_string(),
        },
    }
}

/// The sentence for a format the release does not carry for this
/// architecture. No file name in it, by construction.
fn none_for(format: Format, latest: &Version, arch: &str) -> String {
    format!(
        "BAP Store {latest} has no {} for {} on the releases page, so this copy stays as it is \
         until one is published.",
        format.label(),
        arch_word(arch)
    )
}

/// How a sentence names the architecture: the word the machine gave, or
/// "this machine" when it gave none, so a sentence never carries a gap
/// where a word should be.
fn arch_word(arch: &str) -> &str {
    let arch = arch.trim();
    if arch.is_empty() {
        "this machine"
    } else {
        arch
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(text: &str) -> Version {
        Version::parse(text).expect("a version")
    }

    #[test]
    fn the_documented_names_match_their_own_patterns() {
        // `Format::name` is what the workflow must write and `matches` is
        // what the check reads; they are two spellings of one agreement.
        for format in [
            Format::Deb,
            Format::Rpm,
            Format::PacmanPackage,
            Format::AppImage,
            Format::Flatpak,
        ] {
            for arch in [Arch::X86_64, Arch::Aarch64] {
                let name = format.name(&v("0.2.0"), arch);
                assert!(format.matches(arch, &name), "{format:?} {arch:?}: {name}");
                let other = match arch {
                    Arch::X86_64 => Arch::Aarch64,
                    Arch::Aarch64 => Arch::X86_64,
                };
                assert!(
                    !format.matches(other, &name),
                    "{format:?} {arch:?} matched the other architecture"
                );
            }
        }
    }

    #[test]
    fn a_helper_package_is_not_the_store() {
        assert!(!Format::Deb.matches(Arch::X86_64, "bap-helper_0.2.0_amd64.deb"));
        assert!(
            !Format::PacmanPackage.matches(Arch::X86_64, "bap-store-0.2.0-1-x86_64.pkg.tar.zst")
        );
        assert!(!Format::AppImage.matches(Arch::X86_64, "bap-store-0.2.0-x86_64.AppImage"));
    }

    #[test]
    fn the_architecture_spellings_are_the_managers_own() {
        assert_eq!(Arch::parse("x86_64"), Some(Arch::X86_64));
        assert_eq!(Arch::parse("amd64"), Some(Arch::X86_64));
        assert_eq!(Arch::parse("aarch64"), Some(Arch::Aarch64));
        assert_eq!(Arch::parse("arm64"), Some(Arch::Aarch64));
        assert_eq!(Arch::parse("riscv64"), None);
        assert_eq!(Arch::parse(""), None);
    }

    #[test]
    fn an_asset_on_plain_http_is_treated_as_absent() {
        let assets = [Asset {
            name: "bap-store_0.2.0_amd64.deb".into(),
            browser_download_url: "http://github.com/x/bap-store_0.2.0_amd64.deb".into(),
            size: 1,
        }];
        assert_eq!(find(&assets, Format::Deb, Some(Arch::X86_64)), None);
        let r = remedy(
            &Installation::Dpkg { archive: false },
            &v("0.2.0"),
            "x86_64",
            &assets,
        );
        assert!(matches!(r, Remedy::Sentence { .. }), "{r:?}");
        assert!(!r.sentence().contains(".deb"), "{}", r.sentence());
    }

    #[test]
    fn a_remedy_serialises_with_a_lower_case_kind_and_a_sentence() {
        let r = Remedy::InstallAsset {
            asset: "a".into(),
            url: "https://x/a".into(),
            installer: Installer::PacmanU,
            sentence: "S.".into(),
        };
        let json = serde_json::to_value(&r).expect("serialises");
        assert_eq!(json["kind"], "installasset");
        assert_eq!(json["installer"], "pacmanu");
        assert_eq!(json["sentence"], "S.");
        let s = Remedy::Sentence {
            sentence: "S.".into(),
        };
        let json = serde_json::to_value(&s).expect("serialises");
        assert_eq!(json["kind"], "sentence");
        assert_eq!(json["sentence"], "S.");
        let back: Remedy = serde_json::from_value(json).expect("deserialises");
        assert_eq!(back, s);
    }
}
