//! The Drivers page's data: the driver manager's devices with their
//! profiles, and fwupd's devices with their pending updates.
//!
//! Both halves are asked directly, through `chwd::devices` and
//! `fwupd::devices`, rather than through the `Source` trait: the trait
//! speaks in packages and this page speaks in devices. Downcasting a
//! `dyn Source` back to its module was the alternative, and it would have
//! made every source carry `Any` for the sake of two. Availability still
//! comes from the store's sources, so the page and the sources list agree
//! on why something is missing.

use crate::model::*;
use crate::sources::{chwd, fwupd};
use crate::{Source, Store};

pub fn report(store: &Store) -> DriversReport {
    let mut report = DriversReport::default();

    let manager = status(store, SourceKind::Chwd, || {
        chwd::Chwd::new(&store.system).status()
    });
    if manager.available {
        report.manager = Some("chwd".to_string());
        match chwd::devices() {
            Ok(devices) if devices.is_empty() => {
                report.manager_note =
                    Some("chwd found no device that takes a driver profile.".to_string());
            }
            Ok(devices) => report.devices = devices,
            Err(e) => report.manager_note = Some(e.message),
        }
    } else {
        report.manager_note = Some(
            "This distribution has no driver manager BAP Store can drive. Drivers install as ordinary packages."
                .to_string(),
        );
    }

    let firmware = status(store, SourceKind::Fwupd, || {
        fwupd::Fwupd::new(&store.system).status()
    });
    if firmware.available {
        report.firmware_available = true;
        match fwupd::devices() {
            Ok(devices) if devices.is_empty() => {
                report.firmware_note =
                    Some("fwupd found no device with firmware it can update.".to_string());
            }
            Ok(devices) => report.firmware = devices,
            Err(e) => report.firmware_note = Some(e.message),
        }
    } else {
        report.firmware_note = firmware.reason;
    }

    report
}

/// The store's own status for the source, so the page never disagrees with
/// the sources list; a fresh probe only when the store has no such source.
fn status(store: &Store, kind: SourceKind, probe: impl FnOnce() -> SourceStatus) -> SourceStatus {
    store.source(kind).map(|s| s.status()).unwrap_or_else(probe)
}
