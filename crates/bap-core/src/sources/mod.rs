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
    preferences: &crate::Preferences,
) -> Vec<Box<dyn Source>> {
    let mut aur = aur::Aur::new(system, client.clone(), catalogue.clone());
    // Automatic stays lazy: the helper is looked for on first use. A named
    // choice is applied now, falling back to what is installed when the
    // named one is not, so the setting never points at a missing program.
    if let Some(choice) = preferences.aur_helper.as_deref() {
        aur = aur.with_helper(aur::Helper::choose(choice));
    }
    let mut flatpak = flatpak::Flatpak::new(system, client.clone());
    if preferences.flatpak_user {
        flatpak.installation = flatpak::Installation::User;
    }
    vec![
        Box::new(pacman::Pacman::new(system, catalogue.clone())),
        Box::new(aur),
        Box::new(flatpak),
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
