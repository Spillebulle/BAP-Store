//! The two self-update commands' view of `bap_core::selfupdate`: the check,
//! the plan for its remedy, and the value to draw when no check was made.
//!
//! The shape is Muster's `update::install` transcribed: `installation` says
//! how this copy was installed, `latest` the newest release GitHub reports,
//! and `remedy` the one true way to get it, or `None` when there is none to
//! offer. The page draws `remedy` as an action and never a command that
//! cannot work.

use bap_core::http::Client;
use bap_core::selfupdate::{Probe, Version, assemble, detect};
use bap_core::{Plan, Step};

pub use bap_core::selfupdate::SelfUpdate;

/// The answer when no check has been made: this version, this
/// installation, nothing newer known, nothing to offer. Used when the
/// setting forbids an unasked request, so the page still has a value to
/// draw.
pub fn unchecked() -> SelfUpdate {
    assemble(
        &Version::current(),
        None,
        detect(&Probe::current()),
        std::env::consts::ARCH,
        None,
    )
}

/// Ask whether a newer BAP Store exists and how this copy would get it.
/// Infallible: a failed request is reported inside the value (`error`), so
/// the page can say why.
pub fn check() -> Result<SelfUpdate, String> {
    Ok(bap_core::selfupdate::check(
        &Client::shared(),
        &Probe::current(),
    ))
}

/// Turn a check's remedy into a plan the transaction runner can execute. A
/// remedy that is a row on the Updates page or a sentence has no steps, and
/// the error carries the sentence to show instead.
pub fn plan(update: &SelfUpdate) -> Result<Plan, String> {
    let Some(remedy) = update.remedy.as_ref() else {
        return Err(match &update.latest {
            Some(latest) if latest.newer => "A newer BAP Store exists but this copy cannot be updated from inside the application. The check says where to get it.".to_string(),
            _ => "BAP Store is up to date. There is nothing to apply.".to_string(),
        });
    };
    let steps = bap_core::selfupdate::plan(remedy, &bap_core::selfupdate::download_dir())
        .map_err(|e| e.message)?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_unchecked_answer_is_this_version_with_nothing_to_offer() {
        let u = unchecked();
        assert_eq!(u.current, env!("CARGO_PKG_VERSION"));
        assert!(u.latest.is_none());
        assert!(u.remedy.is_none());
        assert!(u.error.is_none());
    }

    #[test]
    fn the_json_shape_is_the_contract() {
        let json = serde_json::to_value(unchecked()).unwrap();
        for key in ["current", "latest", "installation", "remedy"] {
            assert!(json.get(key).is_some(), "missing {key}");
        }
        assert!(json["installation"]["kind"].is_string());
    }

    #[test]
    fn applying_without_a_remedy_says_why_not() {
        let err = plan(&unchecked()).unwrap_err();
        assert!(err.starts_with("BAP Store is up to date"), "{err}");
    }

    #[test]
    fn a_wrapped_plan_has_its_own_id_and_no_ops() {
        let p = wrap(Vec::new());
        assert!(p.id.starts_with("plan-"));
        assert!(p.ops.is_empty());
    }
}
