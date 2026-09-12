//! One module per source. [`all`] builds them in interface order; each one
//! reports its own availability, so an unavailable source stays in the list
//! and the page can say why.

pub mod alpmdb;
pub mod apt;
pub mod aur;
pub mod chwd;
pub mod dnf;
pub mod flatpak;
pub mod fwupd;
pub mod github;
pub mod pacman;
pub mod snap;

use crate::Source;
use crate::appstream::Catalogue;
use crate::http::Client;
use crate::model::SystemInfo;
use std::sync::Arc;

pub fn all(
    system: &SystemInfo,
    client: Arc<Client>,
    catalogue: Arc<Catalogue>,
) -> Vec<Box<dyn Source>> {
    vec![
        Box::new(pacman::Pacman::new(system, catalogue.clone())),
        Box::new(aur::Aur::new(system, client.clone(), catalogue.clone())),
        Box::new(flatpak::Flatpak::new(system, client.clone())),
        Box::new(snap::Snap::new(system, client.clone())),
        Box::new(apt::Apt::new(system, catalogue.clone())),
        Box::new(dnf::Dnf::new(system, catalogue.clone())),
        Box::new(github::Github::new(
            system,
            client.clone(),
            catalogue.clone(),
        )),
        Box::new(fwupd::Fwupd::new(system)),
        Box::new(chwd::Chwd::new(system)),
    ]
}

/// The status a stub reports until its module is written.
pub(crate) fn not_built(kind: crate::SourceKind) -> crate::SourceStatus {
    crate::SourceStatus {
        kind,
        available: false,
        reason: Some(format!("The {} source is not built yet.", kind.label())),
        detail: None,
    }
}
