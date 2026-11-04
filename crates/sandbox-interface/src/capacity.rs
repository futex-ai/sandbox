//! Typed provider size selection and advisory account capacity offers.

/// Requested provider size or minimum resource amounts for one sandbox.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SandboxSizeSelection {
    /// Opaque size name interpreted only by the selected provider.
    ProviderSize(String),
    /// Minimum resources; an accepted start must provide at least these amounts.
    Resources(SandboxResources),
}

/// CPU, memory, and disk amounts for a requested or offered sandbox.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SandboxResources {
    /// Number of virtual CPU cores.
    pub vcpus: u32,
    /// Memory amount with its explicit unit.
    pub memory: ResourceAmount,
    /// Disk amount with its explicit unit.
    pub disk: ResourceAmount,
}

/// An exact integer resource amount in a specified unit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResourceAmount {
    /// Number of units.
    pub amount: u64,
    /// Unit used by this amount, without implicit decimal/binary conversion.
    pub unit: ResourceUnit,
}

/// Explicit decimal and binary units used for memory and disk.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResourceUnit {
    /// One byte.
    Bytes,
    /// One million bytes.
    Megabytes,
    /// One billion bytes.
    Gigabytes,
    /// 1,048,576 bytes.
    Mebibytes,
    /// 1,073,741,824 bytes.
    Gibibytes,
}

/// One provider size currently eligible for the configured account.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SandboxSizeOffer {
    /// Opaque provider size name accepted by size selection.
    pub provider_size: String,
    /// Published resources for this size; unknown values must not be invented.
    pub resources: SandboxResources,
    /// Exact published hourly price; this offer does not reserve capacity.
    pub hourly_price: SandboxPrice,
}

/// Exact nonnegative decimal amount without binary floating point.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExactDecimal {
    /// Integer coefficient, so 18 at scale 3 represents 0.018.
    pub coefficient: u64,
    /// Number of decimal places: the value is coefficient / 10^scale.
    pub scale: u8,
}

/// A price with an exact amount, explicit currency, and billing unit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SandboxPrice {
    /// Exact amount charged per billing unit.
    pub amount: ExactDecimal,
    /// ISO 4217 currency code, for example `USD`.
    pub currency: String,
    /// Unit for the quoted rate, independent of provider charge granularity.
    pub unit: SandboxPriceUnit,
}

/// Unit in which a sandbox rate is quoted.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SandboxPriceUnit {
    /// Rate for one hour; a provider may charge fractions per second.
    Hour,
}
