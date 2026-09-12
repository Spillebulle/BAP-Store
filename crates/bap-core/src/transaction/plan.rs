//! Operations to steps. TODO: batching of root steps per source into one
//! helper invocation; this stub concatenates what each source says.

use crate::model::*;
use crate::{Result, Store};

pub fn build(store: &Store, ops: &[Op]) -> Result<Plan> {
    let mut steps = Vec::new();
    for op in ops {
        let kind = match op {
            Op::Install { package } | Op::Remove { package } | Op::Update { package } => {
                package.source
            }
            Op::UpdateAll { source } | Op::Refresh { source } => *source,
        };
        let Some(source) = store.source(kind) else {
            return Err(crate::Error::new(format!(
                "{} is not a source on this machine.",
                kind.label()
            )));
        };
        steps.extend(source.plan(op)?);
    }
    Ok(Plan {
        id: new_id(),
        ops: ops.to_vec(),
        steps,
    })
}

pub fn new_id() -> String {
    let t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    format!("plan-{t}")
}
