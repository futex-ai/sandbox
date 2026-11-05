//! Published prices, account eligibility, and exact minimum-resource selection.

use sandbox_interface::{
    Error, ExactDecimal, ResourceAmount, ResourceUnit, SandboxAccountCapacity, SandboxLifecycle,
    SandboxPriceUnit, SandboxResources, SandboxSizeSelection,
};
use serde_json::json;

use super::support::{fixture::Fixture, requests};

#[tokio::test]
async fn capacity_filters_eligible_sizes_and_keeps_exact_published_resources_and_prices() {
    let fixture = Fixture::start().await;
    fixture.state.lock().unwrap().machine_types = json!([
        {"type":"xlarge","available":true,"reason":null}, {"type":"default","available":true,"reason":null},
        {"type":"small","available":true,"reason":null}, {"type":"large","available":true,"reason":null},
        {"type":"future","available":false,"reason":"unsupported"}
    ]);
    let offers = fixture.backend.account_capacity().await.unwrap();
    assert_eq!(
        offers
            .iter()
            .map(|offer| offer.provider_size.as_str())
            .collect::<Vec<_>>(),
        ["small", "default", "large", "xlarge"]
    );
    for (offer, vcpus, memory, disk, price) in [
        (&offers[0], 2, 4, 51, 18),
        (&offers[1], 4, 8, 51, 36),
        (&offers[2], 8, 16, 111, 72),
        (&offers[3], 16, 32, 251, 200),
    ] {
        assert_eq!(offer.resources.vcpus, vcpus);
        assert_eq!(
            offer.resources.memory,
            ResourceAmount {
                amount: memory,
                unit: ResourceUnit::Gigabytes
            }
        );
        assert_eq!(
            offer.resources.disk,
            ResourceAmount {
                amount: disk,
                unit: ResourceUnit::Gigabytes
            }
        );
        assert_eq!(
            offer.hourly_price.amount,
            ExactDecimal {
                coefficient: price,
                scale: 3
            }
        );
        assert_eq!(offer.hourly_price.currency, "USD");
        assert_eq!(offer.hourly_price.unit, SandboxPriceUnit::Hour);
    }
    fixture.state.lock().unwrap().machine_types[1]["available"] = false.into();
    let eligible = fixture.backend.account_capacity().await.unwrap();
    assert_eq!(
        eligible
            .iter()
            .map(|offer| offer.provider_size.as_str())
            .collect::<Vec<_>>(),
        ["small", "large", "xlarge"]
    );
    fixture
        .backend
        .create_sandbox(requests::create())
        .await
        .unwrap();
    assert_eq!(
        fixture
            .captured()
            .iter()
            .map(|call| call.method.as_str())
            .collect::<Vec<_>>(),
        ["GET", "GET", "POST"],
        "canStart=false is advisory"
    );
    fixture.state.lock().unwrap().machine_types =
        json!([{"type":"future","available":true,"reason":null}]);
    assert!(matches!(
        fixture.backend.account_capacity().await,
        Err(Error::Internal(_))
    ));
}

#[tokio::test]
async fn resource_selection_uses_explicit_units_without_downsizing_or_preflight() {
    let fixture = Fixture::start().await;
    let mut request = requests::create();
    request.size = Some(SandboxSizeSelection::Resources(SandboxResources {
        vcpus: 4,
        memory: ResourceAmount {
            amount: 8,
            unit: ResourceUnit::Gibibytes,
        },
        disk: ResourceAmount {
            amount: 51_000,
            unit: ResourceUnit::Megabytes,
        },
    }));
    fixture
        .backend
        .create_sandbox(request.clone())
        .await
        .unwrap();
    assert_eq!(
        fixture.captured()[0].body["type"],
        "large",
        "8 GiB exceeds the default size's 8 GB"
    );
    assert_eq!(fixture.captured().len(), 1);
    let recovered = fixture
        .backend
        .recover_sandbox_create(request)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(recovered.provider_ref.as_str(), "bx_22222222");
    let mut small = requests::create();
    small.size = Some(SandboxSizeSelection::ProviderSize("small".to_owned()));
    fixture.backend.create_sandbox(small).await.unwrap();
    assert_eq!(fixture.captured().last().unwrap().body["type"], "small");
    let mut excessive = requests::create();
    excessive.size = Some(SandboxSizeSelection::Resources(SandboxResources {
        vcpus: 16,
        memory: ResourceAmount {
            amount: u64::MAX,
            unit: ResourceUnit::Gibibytes,
        },
        disk: ResourceAmount {
            amount: 1,
            unit: ResourceUnit::Bytes,
        },
    }));
    assert!(matches!(
        fixture.backend.create_sandbox(excessive).await,
        Err(Error::UnsupportedSizeSelection { .. })
    ));
    assert_eq!(
        fixture
            .captured()
            .iter()
            .filter(|call| call.method == "POST")
            .count(),
        2
    );
}
