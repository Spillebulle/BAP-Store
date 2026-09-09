//! What the application holds between commands.

use bap_core::Store;
use std::sync::{Arc, Mutex, OnceLock};

pub struct AppState {
    store: OnceLock<Arc<Store>>,
    pub settings: Mutex<crate::settings::Settings>,
}

impl AppState {
    pub fn new() -> AppState {
        AppState {
            store: OnceLock::new(),
            settings: Mutex::new(crate::settings::Settings::load()),
        }
    }

    /// The store, detected on first use so the window opens before the
    /// package databases are read.
    pub fn store(&self) -> Arc<Store> {
        self.store.get_or_init(|| Arc::new(Store::detect())).clone()
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
