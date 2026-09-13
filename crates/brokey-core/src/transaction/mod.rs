//! Plans and the runner: the privilege boundary of the whole application.
//!
//! - `plan` turns operations into steps, ordered so one helper run covers as
//!   many root steps as possible, with consecutive package-manager calls
//!   joined into one.
//! - `allow` is the closed list of what may run as root. It is a pure
//!   function of the plan; `brokey-helper` calls it and so does the runner's
//!   dry run.
//! - `progress` reads a tool's output for "(3/7) installing foo".
//! - `runner` executes a plan: root steps through `pkexec brokey-helper run`,
//!   session steps here, every line an event. The only `pkexec` call site.

pub mod allow;
pub mod plan;
pub mod progress;
pub mod runner;

pub use allow::{Allowed, validate, validate_with};
pub use plan::{PARTIAL_UPGRADE_NOTICE, build, log_out_notice, notices, notices_in};
pub use progress::{ProgressParser, Reading};
pub use runner::{CancelToken, Outcome, Runner, Sink};
