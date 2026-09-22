//! Stable consumer-owned runtime identifiers.

use std::{fmt, str::FromStr};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

macro_rules! runtime_id {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(
            Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
        )]
        #[serde(transparent)]
        pub struct $name(Uuid);

        impl $name {
            #[doc = "Allocates a new UUIDv7 identifier."]
            #[must_use]
            pub fn new() -> Self {
                Self(Uuid::now_v7())
            }

            #[doc = "Wraps an existing UUID."]
            #[must_use]
            pub const fn from_uuid(value: Uuid) -> Self {
                Self(value)
            }

            #[doc = "Returns the wrapped UUID."]
            #[must_use]
            pub const fn as_uuid(self) -> Uuid {
                self.0
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(formatter)
            }
        }

        impl FromStr for $name {
            type Err = uuid::Error;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                Uuid::parse_str(value).map(Self)
            }
        }
    };
}

runtime_id!(SandboxId, "Stable consumer sandbox identifier.");
runtime_id!(SnapshotId, "Stable consumer snapshot identifier.");
runtime_id!(TerminalId, "Stable consumer terminal identifier.");
runtime_id!(ActionId, "Stable consumer terminal-action identifier.");
runtime_id!(OperationId, "Stable durable tool-operation identifier.");
