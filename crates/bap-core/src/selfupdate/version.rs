//! Release versions, and the tags that carry them.
//!
//! The version lives in one place, `[workspace.package]` in the root
//! manifest, and a release is the tag `v<version>`. Comparing versions as
//! text is the classic way to get an updater wrong: in every lexical
//! ordering `"0.0.10"` sorts before `"0.0.9"`, so the tenth patch release
//! would look older than the ninth and never be offered. The parts are
//! parsed into numbers and compared as numbers.
//!
//! A pre-release (`0.2.0-rc.1`) is parsed rather than refused, unlike
//! Muster, because a pre-release tag is something this repository may push
//! while the packaging is being proved on Debian and Fedora. It sorts below
//! the release it precedes, which is what keeps a stable copy from being
//! offered a candidate: `/releases/latest` never returns a pre-release, and
//! if it ever did, a candidate is still never newer than the release it
//! leads to.

use std::cmp::Ordering;
use std::fmt;

/// A `major.minor.patch` version with an optional pre-release suffix.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Version {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
    /// The identifiers after the hyphen, `rc.1` in `0.2.0-rc.1`. `None` is a
    /// release proper, which outranks every candidate for the same numbers.
    pub pre: Option<String>,
}

impl Version {
    /// The version this build was compiled as.
    ///
    /// `CARGO_PKG_VERSION` comes from the workspace manifest, and the release
    /// tests already pin that to the changelog, so a build which passes CI
    /// cannot have a version this fails to parse. The fallback exists so a
    /// mistake is a version that never offers an update rather than a panic
    /// on start-up.
    pub fn current() -> Version {
        Version::parse(env!("CARGO_PKG_VERSION")).unwrap_or(Version {
            major: 0,
            minor: 0,
            patch: 0,
            pre: None,
        })
    }

    /// Parse `0.1.0`, `v0.1.0` or `v0.2.0-rc.1`. Exactly three all-digit
    /// parts; an optional pre-release of dot-separated identifiers; nothing
    /// else. Build metadata (`+build.5`) is refused rather than ignored: the
    /// release workflow never writes it, and a tag carrying it was not
    /// written by that workflow.
    pub fn parse(text: &str) -> Option<Version> {
        let text = text.trim();
        let text = text.strip_prefix('v').unwrap_or(text);
        if text.contains('+') {
            return None;
        }
        let (core, pre) = match text.split_once('-') {
            Some((core, pre)) => (core, Some(pre)),
            None => (text, None),
        };
        let mut parts = core.split('.');
        let major = number(parts.next()?)?;
        let minor = number(parts.next()?)?;
        let patch = number(parts.next()?)?;
        // `1.2.3.4` is not a version this project produces, and treating it
        // as `1.2.3` would silently accept a tag nobody here wrote.
        if parts.next().is_some() {
            return None;
        }
        let pre = match pre {
            Some(pre) => {
                let well_formed = !pre.is_empty()
                    && pre.split('.').all(|id| {
                        !id.is_empty() && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
                    });
                if !well_formed {
                    return None;
                }
                Some(pre.to_string())
            }
            None => None,
        };
        Some(Version {
            major,
            minor,
            patch,
            pre,
        })
    }

    /// Whether this is a release proper rather than a candidate.
    pub fn is_release(&self) -> bool {
        self.pre.is_none()
    }
}

/// One numeric part of a version.
///
/// Hand-checked rather than left to `u64::from_str`, which accepts a leading
/// `+` (so `1.+2.3` would parse) and because `split` yields an empty string
/// for the middle of `1..2`, which must parse as nothing at all.
fn number(part: &str) -> Option<u64> {
    if part.is_empty() || !part.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    part.parse().ok()
}

/// Semantic-versioning precedence for two pre-release strings: identifier by
/// identifier, numbers as numbers and below words, and a longer list of
/// otherwise equal identifiers is the higher.
fn pre_cmp(a: &str, b: &str) -> Ordering {
    let mut left = a.split('.');
    let mut right = b.split('.');
    loop {
        match (left.next(), right.next()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(x), Some(y)) => {
                let order = match (number(x), number(y)) {
                    (Some(m), Some(n)) => m.cmp(&n),
                    (Some(_), None) => Ordering::Less,
                    (None, Some(_)) => Ordering::Greater,
                    (None, None) => x.cmp(y),
                };
                if order != Ordering::Equal {
                    return order;
                }
            }
        }
    }
}

impl Ord for Version {
    fn cmp(&self, other: &Version) -> Ordering {
        (self.major, self.minor, self.patch)
            .cmp(&(other.major, other.minor, other.patch))
            .then_with(|| match (&self.pre, &other.pre) {
                (None, None) => Ordering::Equal,
                // A derived Ord would put `None` first; a release has to
                // outrank its own candidates, so the order is by hand.
                (None, Some(_)) => Ordering::Greater,
                (Some(_), None) => Ordering::Less,
                (Some(a), Some(b)) => pre_cmp(a, b),
            })
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Version) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)?;
        if let Some(pre) = &self.pre {
            write!(f, "-{pre}")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(major: u64, minor: u64, patch: u64) -> Version {
        Version {
            major,
            minor,
            patch,
            pre: None,
        }
    }

    fn pre(major: u64, minor: u64, patch: u64, pre: &str) -> Version {
        Version {
            major,
            minor,
            patch,
            pre: Some(pre.to_string()),
        }
    }

    #[test]
    fn a_version_is_compared_as_numbers_not_as_text() {
        assert!(v(0, 0, 10) > v(0, 0, 9));
        assert!("0.0.10" < "0.0.9", "the trap this guards against");
        assert!(v(0, 10, 0) > v(0, 9, 0));
        assert!(v(10, 0, 0) > v(9, 0, 0));
        assert!(v(1, 0, 0) > v(0, 99, 99));
        assert!(v(0, 2, 0) > v(0, 1, 99));
        assert_eq!(v(1, 2, 3), v(1, 2, 3));
    }

    #[test]
    fn a_candidate_sorts_below_the_release_it_precedes() {
        assert!(pre(0, 2, 0, "rc.1") < v(0, 2, 0));
        assert!(pre(0, 2, 0, "rc.1") > v(0, 1, 9));
        assert!(pre(0, 2, 0, "rc.2") > pre(0, 2, 0, "rc.1"));
        assert!(pre(0, 2, 0, "rc.10") > pre(0, 2, 0, "rc.9"));
        assert!(pre(0, 2, 0, "beta") > pre(0, 2, 0, "alpha"));
        assert!(pre(0, 2, 0, "alpha.1") > pre(0, 2, 0, "alpha"));
        // Numbers rank below words, as the specification says.
        assert!(pre(0, 2, 0, "1") < pre(0, 2, 0, "alpha"));
    }

    #[test]
    fn the_tags_the_release_workflow_pushes_parse() {
        assert_eq!(Version::parse("v0.0.1"), Some(v(0, 0, 1)));
        assert_eq!(Version::parse("v0.0.10"), Some(v(0, 0, 10)));
        assert_eq!(Version::parse("0.1.0"), Some(v(0, 1, 0)));
        assert_eq!(Version::parse("v12.34.56"), Some(v(12, 34, 56)));
        assert_eq!(Version::parse("v0.2.0-rc.1"), Some(pre(0, 2, 0, "rc.1")));
        assert_eq!(Version::parse(" v0.1.0 "), Some(v(0, 1, 0)));
    }

    #[test]
    fn a_tag_that_is_not_ours_is_ignored_rather_than_misparsed() {
        for tag in [
            "nightly",
            "latest",
            "v2-beta",
            "v0.1",
            "v0.1.0.1",
            "v0.1.0+build",
            "vv0.1.0",
            "v0.1.x",
            "v 0.1.0",
            "v+1.0.0",
            "v-1.0.0",
            "v1..0",
            "v1.0.0-",
            "v1.0.0-rc..1",
            "v1.0.0-rc 1",
            "v",
            "",
        ] {
            assert_eq!(Version::parse(tag), None, "{tag:?} was parsed");
        }
    }

    #[test]
    fn a_version_round_trips_through_its_own_display() {
        for version in [v(0, 0, 1), v(0, 0, 10), v(1, 20, 300), pre(0, 2, 0, "rc.1")] {
            assert_eq!(Version::parse(&version.to_string()), Some(version));
        }
    }

    #[test]
    fn this_build_knows_its_own_version() {
        assert_eq!(
            Version::current(),
            Version::parse(env!("CARGO_PKG_VERSION")).expect("the crate version parses")
        );
        assert!(Version::current().is_release());
    }
}
