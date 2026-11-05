//! Boat's published catalogue, verified on 4 November 2026.

use sandbox_interface::{
    ExactDecimal, ResourceAmount, ResourceUnit, SandboxPrice, SandboxPriceUnit, SandboxResources,
    SandboxSizeOffer, SandboxSizeSelection,
};

/// Published sizes ordered from smallest to largest.
pub const MACHINE_TYPES: [&str; 4] = ["small", "default", "large", "xlarge"];

/// Returns published GB resources and the exact USD hourly rate for a size.
#[must_use]
pub fn offer(machine_type: &str) -> Option<SandboxSizeOffer> {
    let (vcpus, memory, disk, coefficient) = match machine_type {
        "small" => (2, 4, 51, 18),
        "default" => (4, 8, 51, 36),
        "large" => (8, 16, 111, 72),
        "xlarge" => (16, 32, 251, 200),
        _ => return None,
    };
    Some(SandboxSizeOffer {
        provider_size: machine_type.to_owned(),
        resources: SandboxResources {
            vcpus,
            memory: ResourceAmount {
                amount: memory,
                unit: ResourceUnit::Gigabytes,
            },
            disk: ResourceAmount {
                amount: disk,
                unit: ResourceUnit::Gigabytes,
            },
        },
        hourly_price: SandboxPrice {
            amount: ExactDecimal {
                coefficient,
                scale: 3,
            },
            currency: "USD".to_owned(),
            unit: SandboxPriceUnit::Hour,
        },
    })
}

pub(crate) fn select(selection: &SandboxSizeSelection) -> Option<String> {
    match selection {
        SandboxSizeSelection::ProviderSize(name) => offer(name).map(|offer| offer.provider_size),
        SandboxSizeSelection::Resources(request) => MACHINE_TYPES
            .iter()
            .filter_map(|name| offer(name))
            .find(|offer| {
                offer.resources.vcpus >= request.vcpus
                    && bytes(offer.resources.memory) >= bytes(request.memory)
                    && bytes(offer.resources.disk) >= bytes(request.disk)
            })
            .map(|offer| offer.provider_size),
    }
}

fn bytes(amount: ResourceAmount) -> u128 {
    let factor = match amount.unit {
        ResourceUnit::Bytes => 1,
        ResourceUnit::Megabytes => 1_000_000,
        ResourceUnit::Gigabytes => 1_000_000_000,
        ResourceUnit::Mebibytes => 1_048_576,
        ResourceUnit::Gibibytes => 1_073_741_824,
    };
    u128::from(amount.amount) * factor
}
