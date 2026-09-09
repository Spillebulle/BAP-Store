//! Plans and the runner. `plan` turns operations into steps grouped by
//! source and privilege; `runner` executes a plan and streams events.

pub mod plan;
pub mod runner;
