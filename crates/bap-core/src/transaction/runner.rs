//! Executes a plan. TODO: the helper hand-off, streaming, cancellation.

use crate::model::*;

/// Receives events as a plan runs.
pub trait Sink: Send {
    fn event(&mut self, event: Event);
}

impl<F: FnMut(Event) + Send> Sink for F {
    fn event(&mut self, event: Event) {
        self(event)
    }
}

pub struct Runner;

impl Runner {
    pub fn run(_plan: &Plan, _sink: &mut dyn Sink) -> crate::Result<()> {
        Err(crate::Error::new("The transaction runner is not built yet."))
    }
}
