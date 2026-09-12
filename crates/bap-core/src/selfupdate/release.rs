//! What GitHub says the newest release of this repository is.
//!
//! One request, to `/releases/latest`, which GitHub defines as the newest
//! release that is neither a draft nor a pre-release. That is the right
//! answer for an updater: a stable copy is never walked onto a candidate
//! build by an automatic check. The reply is read with serde and every field
//! this module does not use is dropped; a release payload carries a hundred
//! keys and the check needs seven.
//!
//! **Nothing here builds a download URL.** The address of an asset is taken
//! from the reply verbatim and must be `https`. Constructing one from a
//! version number would mean this code deciding where a binary is fetched
//! from, which is exactly the decision that belongs to the release the API
//! just described.

use super::version::Version;
use crate::http::Client;
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// The repository, as shown to the user.
pub const REPOSITORY: &str = "https://github.com/Spillebulle/BAP-Store";

/// Where somebody goes to fetch a build by hand, which is the answer for
/// every installation BAP Store may not update itself.
pub const RELEASES_PAGE: &str = "https://github.com/Spillebulle/BAP-Store/releases";

/// The one request this module makes.
pub const API_LATEST: &str = "https://api.github.com/repos/Spillebulle/BAP-Store/releases/latest";

/// How long an answer is trusted before GitHub is asked again. Six hours:
/// a release is a rare event, GitHub's unauthenticated limit is sixty
/// requests an hour, and the application's own ten-minute cache sits in
/// front of this one.
pub const CACHE_FOR: Duration = Duration::from_secs(6 * 60 * 60);

/// One file attached to a release.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Asset {
    pub name: String,
    pub browser_download_url: String,
    /// The size GitHub recorded on upload, so a download can be checked
    /// against it and the page can say how big it is.
    #[serde(default)]
    pub size: u64,
}

impl Asset {
    /// Whether this is an address BAP Store is willing to fetch from.
    ///
    /// Releases are not signed, so the transport is the whole of the
    /// guarantee: TLS to a host GitHub controls. An asset on plain `http`
    /// would throw away even that, so it is treated as though it were not
    /// there.
    pub fn is_fetchable(&self) -> bool {
        self.browser_download_url.starts_with("https://")
    }
}

/// A release, reduced to what the check needs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Release {
    pub version: Version,
    pub tag: String,
    /// The release's title. The workflow sets it to the bare version; a
    /// release with no title at all is called "BAP Store <v>" here.
    pub name: String,
    /// `CHANGELOG.md`'s section for this version, which is what the workflow
    /// publishes as the release body. Markdown.
    pub notes: String,
    /// Unix seconds, from `published_at`.
    pub published: Option<i64>,
    /// The release's own page, for the reader who would rather do it by hand.
    pub url: String,
    pub assets: Vec<Asset>,
}

/// What the API returns, as far as this module reads it. Everything but the
/// tag has a default: the tag is the one field without which there is no
/// release to speak of.
#[derive(Deserialize)]
struct Raw {
    tag_name: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    published_at: Option<String>,
    #[serde(default)]
    html_url: Option<String>,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    assets: Vec<Asset>,
}

/// Read one release object as GitHub sends it.
///
/// `Ok(None)` is a reply that is a release but not one to offer: a draft, a
/// pre-release, or a tag that is not `v<version>`. `/releases/latest` should
/// never send the first two, and a tag like `nightly` is ignored rather than
/// guessed at, because misreading a tag is how an updater comes to offer a
/// release that does not exist. `Err` is a reply that is not a release at
/// all: a rate-limit refusal, a 404 body, or HTML.
pub fn parse(json: &str) -> Result<Option<Release>> {
    let raw: Raw = serde_json::from_str(json).map_err(|e| {
        Error::new(format!(
            "GitHub sent something that was not a release: {e}. Try again later."
        ))
    })?;
    if raw.draft || raw.prerelease {
        return Ok(None);
    }
    let Some(version) = Version::parse(&raw.tag_name) else {
        log::warn!(
            "the newest release is tagged {:?}, which is not a version; ignoring it",
            raw.tag_name
        );
        return Ok(None);
    };
    Ok(Some(Release {
        name: raw
            .name
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| format!("BAP Store {version}")),
        notes: raw.body.unwrap_or_default(),
        published: raw.published_at.as_deref().and_then(epoch_seconds),
        url: raw
            .html_url
            .filter(|u| !u.is_empty())
            .unwrap_or_else(|| RELEASES_PAGE.to_string()),
        version,
        tag: raw.tag_name,
        assets: raw.assets,
    }))
}

/// Ask GitHub for the newest release, through the six-hour disk cache, or
/// past it when `fresh` is set: a check the user asked for by name goes to
/// GitHub whatever the cache holds, and stores what it gets.
///
/// A 404 is a repository with no release yet, which is `Ok(None)`: the page
/// says so rather than showing an error for a state that is expected before
/// the first release. A 403 or 429 is GitHub's unauthenticated limit for
/// this address, sixty requests an hour, and the sentence says so and when
/// to try again. Every other failure is an error in a sentence that names
/// GitHub and says to try again.
///
/// The cache is `Client::get_text_cached`, which carries no headers and no
/// status code, so the statuses are recognised from the sentence `http.rs`
/// writes for them. A typed status would be better; the alternative, a
/// second cache in this module over `Client::raw()`, would duplicate the one
/// that exists.
pub fn latest(client: &Client, fresh: bool) -> Result<Option<Release>> {
    let answer = if fresh {
        client.get_text_fresh(API_LATEST)
    } else {
        client.get_text_cached(API_LATEST, CACHE_FOR)
    };
    match answer {
        Ok(text) => parse(&text),
        Err(e) if is_not_found(&e) => Ok(None),
        Err(e) => Err(describe_failure(&e)),
    }
}

fn is_not_found(e: &Error) -> bool {
    e.message.contains("answered 404")
}

fn is_rate_limited(e: &Error) -> bool {
    e.message.contains("answered 403") || e.message.contains("answered 429")
}

/// The sentence for a request that failed for a reason other than "no
/// release yet".
fn describe_failure(e: &Error) -> Error {
    if is_rate_limited(e) {
        return Error::new(
            "GitHub's request limit for this address is used up, so a newer BAP Store could not be checked for. Try again in an hour.",
        );
    }
    Error::new(format!(
        "Could not check GitHub for a newer BAP Store: {}. Try again later.",
        e.message
    ))
}

/// `2026-09-01T12:34:56Z` as Unix seconds. GitHub's timestamps are always
/// UTC with a `Z`, so no zone handling; the civil-to-days arithmetic is the
/// standard one and is tested against `date`. `None` for anything else.
pub fn epoch_seconds(iso: &str) -> Option<i64> {
    let iso = iso.strip_suffix('Z')?;
    let (date, time) = iso.split_once('T')?;
    let mut d = date.split('-');
    let year: i64 = d.next()?.parse().ok()?;
    let month: u32 = d.next()?.parse().ok()?;
    let day: u32 = d.next()?.parse().ok()?;
    if d.next().is_some() || !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    let mut t = time.split(':');
    let hour: i64 = t.next()?.parse().ok()?;
    let minute: i64 = t.next()?.parse().ok()?;
    let second: i64 = t.next()?.parse().ok()?;
    if t.next().is_some() || hour > 23 || minute > 59 || second > 60 {
        return None;
    }
    Some(days_from_civil(year, month, day) * 86_400 + hour * 3_600 + minute * 60 + second)
}

/// Days since 1970-01-01 for a proleptic Gregorian date.
fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = i64::from((month + 9) % 12);
    let doy = (153 * mp + 2) / 5 + i64::from(day) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timestamps_agree_with_date() {
        // Values from `date -u -d <stamp> +%s`.
        assert_eq!(epoch_seconds("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(epoch_seconds("2000-03-01T00:00:00Z"), Some(951_868_800));
        assert_eq!(epoch_seconds("2024-02-29T23:59:59Z"), Some(1_709_251_199));
        assert_eq!(epoch_seconds("1999-12-31T00:00:00Z"), Some(946_598_400));
        assert_eq!(epoch_seconds("2026-09-01T12:34:56Z"), Some(1_788_266_096));
    }

    #[test]
    fn a_timestamp_that_is_not_utc_iso_is_not_guessed_at() {
        for bad in [
            "",
            "2026-09-01",
            "2026-09-01T12:34:56",
            "2026-09-01T12:34:56+02:00",
            "yesterday",
            "2026-13-01T00:00:00Z",
        ] {
            assert_eq!(epoch_seconds(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn a_body_that_is_absent_reads_as_empty_rather_than_failing() {
        let json = r#"{"tag_name":"v1.0.0","body":null,"html_url":null,"assets":[]}"#;
        let release = parse(json).expect("parses").expect("a release");
        assert_eq!(release.notes, "");
        assert_eq!(release.url, RELEASES_PAGE);
        assert_eq!(release.name, "BAP Store 1.0.0");
        assert_eq!(release.published, None);
    }

    #[test]
    fn drafts_prereleases_and_foreign_tags_are_ignored_not_offered() {
        for json in [
            r#"{"tag_name":"v1.0.0","draft":true}"#,
            r#"{"tag_name":"v1.0.0","prerelease":true}"#,
            r#"{"tag_name":"nightly"}"#,
        ] {
            assert_eq!(parse(json).expect("parses"), None, "{json}");
        }
    }

    #[test]
    fn a_reply_that_is_not_a_release_is_an_error_not_a_guess() {
        // What GitHub sends for a rate limit: an object with a message.
        // Reading that as "no update" would be indistinguishable from being
        // up to date.
        let refusal = r#"{"message":"API rate limit exceeded","documentation_url":"https://..."}"#;
        let err = parse(refusal).expect_err("a refusal is an error");
        assert!(
            err.message
                .starts_with("GitHub sent something that was not a release"),
            "{err}"
        );
        assert!(parse("<html>").is_err());
    }

    #[test]
    fn only_the_404_sentence_reads_as_no_release_yet() {
        assert!(is_not_found(&Error::new(
            "api.github.com answered 404 Not Found"
        )));
        assert!(!is_not_found(&Error::new(
            "api.github.com answered 403 Forbidden"
        )));
        assert!(!is_not_found(&Error::new("could not reach api.github.com")));
    }

    #[test]
    fn a_403_or_429_names_the_request_limit_and_when_to_try_again() {
        for status in ["403 Forbidden", "429 Too Many Requests"] {
            let e = describe_failure(&Error::new(format!("api.github.com answered {status}")));
            assert_eq!(
                e.message,
                "GitHub's request limit for this address is used up, so a newer BAP Store could not be checked for. Try again in an hour."
            );
        }
        // Anything else keeps the reason and says to try again.
        let e = describe_failure(&Error::new("could not reach api.github.com"));
        assert_eq!(
            e.message,
            "Could not check GitHub for a newer BAP Store: could not reach api.github.com. Try again later."
        );
        assert!(
            !describe_failure(&Error::new(
                "api.github.com answered 500 Internal Server Error"
            ))
            .message
            .contains("limit")
        );
    }
}
