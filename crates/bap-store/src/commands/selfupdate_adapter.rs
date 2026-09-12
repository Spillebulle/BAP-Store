//! The two self-update commands' view of `bap_core::selfupdate`, which is
//! being written on another branch. Until it lands this module answers in
//! the same shape with nothing found, so the page and the text mode can be
//! built and run against it; each `TODO(integration)` names the exact call
//! that replaces the stand-in.
//!
//! The shape is Muster's `update::install` transcribed: `installation` is a
//! tagged value naming how this copy was installed, `latest` the newest
//! release GitHub reports if it is newer than this one, and `remedy` the
//! one true way to get it, or `None` when there is none to offer. The page
//! draws `remedy` as an action and never a command that cannot work.

use bap_core::{Plan, Step};
use serde::{Deserialize, Serialize};

/// What a self-update check answers.
///
/// TODO(integration): replace with `bap_core::selfupdate::SelfUpdate` and
/// drop this struct. The field names and their JSON are the contract with
/// the page and must not change.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SelfUpdate {
    /// This build's version.
    pub current: String,
    /// The newest release, when it is newer than `current`.
    pub latest: Option<serde_json::Value>,
    /// How this copy was installed, as `{"kind": ...}`.
    pub installation: serde_json::Value,
    /// How to get `latest` on this machine, or nothing to offer.
    pub remedy: Option<serde_json::Value>,
}

impl SelfUpdate {
    /// The answer when no check has been made: this version, nothing newer
    /// known, nothing to offer. Used when the setting forbids an unasked
    /// request, so the page still has a value to draw.
    pub fn unchecked() -> SelfUpdate {
        SelfUpdate {
            current: env!("CARGO_PKG_VERSION").to_string(),
            latest: None,
            installation: serde_json::json!({ "kind": "unknown" }),
            remedy: None,
        }
    }

    /// Whether there is anything to apply.
    pub fn has_remedy(&self) -> bool {
        self.remedy.is_some()
    }
}

/// Ask whether a newer BAP Store exists and how this copy would get it.
///
/// TODO(integration): swap the body for
/// `bap_core::selfupdate::check(&bap_core::http::Client::shared(), &bap_core::selfupdate::Probe::current())`
/// and return it directly (it is infallible in the contract: a failed
/// request is reported inside the value, so the page can say why).
pub fn check() -> Result<SelfUpdate, String> {
    Ok(SelfUpdate::unchecked())
}

/// Turn a check's remedy into a plan the transaction runner can execute.
///
/// TODO(integration): the steps come from
/// `bap_core::selfupdate::plan(remedy, &download_dir())?` where `remedy` is
/// `&bap_core::selfupdate::Remedy`; wrap them with [`wrap`] as below.
pub fn plan(update: &SelfUpdate) -> Result<Plan, String> {
    let Some(_remedy) = update.remedy.as_ref() else {
        return Err(match &update.latest {
            Some(_) => "A newer BAP Store exists but this copy cannot be updated from inside the application. The check says where to get it.".to_string(),
            None => "BAP Store is up to date. There is nothing to apply.".to_string(),
        });
    };
    let steps: Vec<Step> = Vec::new();
    Ok(wrap(steps))
}

/// A self-update has no `Op` the sources know, so its plan carries the steps
/// alone under a fresh id. The runner does not care; the activity panel
/// draws it by its steps' titles.
pub fn wrap(steps: Vec<Step>) -> Plan {
    Plan {
        id: bap_core::transaction::plan::new_id(),
        ops: Vec::new(),
        steps,
    }
}

/// Where a release asset is downloaded before the helper installs it.
pub fn download_dir() -> std::path::PathBuf {
    bap_core::system::Dirs::new().cache.join("selfupdate")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_stand_in_answers_this_version_with_nothing_to_offer() {
        let u = check().unwrap();
        assert_eq!(u.current, env!("CARGO_PKG_VERSION"));
        assert!(u.latest.is_none());
        assert!(u.remedy.is_none());
        assert!(!u.has_remedy());
    }

    #[test]
    fn the_json_shape_is_the_contract() {
        let json = serde_json::to_value(SelfUpdate::unchecked()).unwrap();
        for key in ["current", "latest", "installation", "remedy"] {
            assert!(json.get(key).is_some(), "missing {key}");
        }
        assert_eq!(json["installation"]["kind"], "unknown");
    }

    #[test]
    fn applying_without_a_remedy_says_why_not() {
        let err = plan(&SelfUpdate::unchecked()).unwrap_err();
        assert!(err.starts_with("BAP Store is up to date"), "{err}");
        let mut newer = SelfUpdate::unchecked();
        newer.latest = Some(serde_json::json!({ "version": "9.9.9" }));
        let err = plan(&newer).unwrap_err();
        assert!(err.contains("cannot be updated from inside"), "{err}");
    }

    #[test]
    fn a_wrapped_plan_has_its_own_id_and_no_ops() {
        let p = wrap(Vec::new());
        assert!(p.id.starts_with("plan-"));
        assert!(p.ops.is_empty());
    }
}
