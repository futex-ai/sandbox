//! Account-filtered published resources and exact prices.

use std::collections::HashSet;

use sandbox_interface::{Result, SandboxSizeOffer};

use crate::{catalogue, error::Error};

use super::{configured::BoatSandboxBackend, mapping};

pub(super) async fn offers(backend: &BoatSandboxBackend) -> Result<Vec<SandboxSizeOffer>> {
    let limits = mapping::control_result(
        backend.control.limits().await,
        backend.config.backend_id(),
        None,
    )?;
    let mut eligible = HashSet::new();
    for entry in limits
        .machine_types
        .into_iter()
        .filter(|entry| entry.available)
    {
        if catalogue::offer(&entry.machine_type).is_none() {
            return mapping::control_result(
                Err(Error::UnknownMachineType),
                backend.config.backend_id(),
                None,
            );
        }
        eligible.insert(entry.machine_type);
    }
    Ok(catalogue::MACHINE_TYPES
        .iter()
        .filter(|name| eligible.contains(**name))
        .filter_map(|name| catalogue::offer(name))
        .collect())
}
