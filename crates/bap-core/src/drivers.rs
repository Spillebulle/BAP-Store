//! The Drivers page's data: devices and their profiles from the driver
//! manager, and firmware from fwupd. TODO: built by the chwd and fwupd
//! source modules' helpers.

use crate::Store;
use crate::model::DriversReport;

pub fn report(_store: &Store) -> DriversReport {
    DriversReport {
        manager: None,
        manager_note: Some("Drivers are not managed on this distribution yet.".to_string()),
        firmware_available: false,
        firmware_note: Some("The firmware source is not built yet.".to_string()),
        ..DriversReport::default()
    }
}
