//! AppStream catalogue: parser, index, icon resolution. TODO: the real
//! parser; this stub is an empty catalogue.

use crate::model::SystemInfo;
use std::sync::Arc;

/// A component from a catalogue, with what the store draws from it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Component {
    pub id: String,
    pub pkgname: Option<String>,
    pub name: String,
    pub summary: Option<String>,
    pub description: Option<String>,
    pub developer: Option<String>,
    pub licence: Option<String>,
    pub homepage: Option<String>,
    pub categories: Vec<String>,
    pub keywords: Vec<String>,
    pub icon: Option<crate::model::Picture>,
    pub screenshots: Vec<crate::model::Screenshot>,
    pub is_app: bool,
}

#[derive(Debug, Default)]
pub struct Catalogue {
    components: Vec<Component>,
}

impl Catalogue {
    /// Every catalogue the distribution has installed.
    pub fn load_system(_system: &SystemInfo) -> Arc<Catalogue> {
        Arc::new(Catalogue::default())
    }

    pub fn by_id(&self, _id: &str) -> Option<&Component> {
        None
    }

    pub fn by_pkgname(&self, _pkgname: &str) -> Option<&Component> {
        None
    }

    pub fn components(&self) -> &[Component] {
        &self.components
    }
}
