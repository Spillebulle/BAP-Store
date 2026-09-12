//! Every source's updates, merged. TODO: the real thing.

use crate::Store;
use crate::model::*;

#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct UpdateList {
    pub updates: Vec<Update>,
    pub failed: Vec<(SourceKind, String)>,
    /// Unix seconds when this list was made.
    pub checked_at: i64,
}

pub fn collect(store: &Store) -> UpdateList {
    let mut list = UpdateList::default();
    for source in &store.sources {
        if !source.status().available {
            continue;
        }
        match source.updates() {
            Ok(mut u) => list.updates.append(&mut u),
            Err(e) => list.failed.push((source.kind(), e.message)),
        }
    }
    list.checked_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    list
}
