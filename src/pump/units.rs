//! Quantities and `Bounds`, monomorphised per unit.
//!
//! The shared contract is generic (`Bounds<T>` over `Quantity`). Generics do
//! not cross the ABI, so there is one record per unit. The raw integer is the
//! unit the shared crate uses, never a float: milliunits, milliunits per
//! hour, whole percent, whole seconds. Converting to the app's own type
//! (`Double` in AAPS, `Decimal` in Trio) is the adapter's job, once, at the
//! edge (guideline 13).
//!
//! The rounding helpers below are not logic added by the bindings: each one
//! calls straight into `pump_shared::Bounds`, so Kotlin and Swift round
//! identically, which is exactly what guideline 13 asks for.

use core::time::Duration;

use pump_shared::{BasalProfile, BasalSegment as PsBasalSegment, ProfileError};
use pump_shared::{
    BasalRate, Bounds, InsulinAmount, Percent, TempBasalRate as PsTempBasalRate, Tier,
};

use crate::error::BindingError;

/// Insulin, in milliunits (1 U = 1000 mU).
pub type Milliunits = u32;
/// A basal rate, in milliunits per hour.
pub type MilliunitsPerHour = u32;

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum TempBasalRate {
    Percent { percent: u16 },
    Absolute { milliunits_per_hour: u32 },
}

impl From<PsTempBasalRate> for TempBasalRate {
    fn from(rate: PsTempBasalRate) -> Self {
        match rate {
            PsTempBasalRate::Percent(percent) => Self::Percent {
                percent: percent.get(),
            },
            PsTempBasalRate::Absolute(rate) => Self::Absolute {
                milliunits_per_hour: rate.milliunits_per_hour(),
            },
        }
    }
}

impl From<TempBasalRate> for PsTempBasalRate {
    fn from(rate: TempBasalRate) -> Self {
        match rate {
            TempBasalRate::Percent { percent } => Self::Percent(Percent::new(percent)),
            TempBasalRate::Absolute {
                milliunits_per_hour,
            } => Self::Absolute(BasalRate::from_milliunits_per_hour(milliunits_per_hour)),
        }
    }
}

/// One segment of a basal profile, starting `start_secs` after midnight.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct BasalSegment {
    pub start_secs: u64,
    pub milliunits_per_hour: u32,
}

impl From<&PsBasalSegment> for BasalSegment {
    fn from(segment: &PsBasalSegment) -> Self {
        Self {
            start_secs: segment.start.as_secs(),
            milliunits_per_hour: segment.rate.milliunits_per_hour(),
        }
    }
}

impl From<BasalSegment> for PsBasalSegment {
    fn from(segment: BasalSegment) -> Self {
        Self {
            start: Duration::from_secs(segment.start_secs),
            rate: BasalRate::from_milliunits_per_hour(segment.milliunits_per_hour),
        }
    }
}

pub(crate) fn profile(segments: Vec<BasalSegment>) -> Result<BasalProfile, BindingError> {
    BasalProfile::new(segments.into_iter().map(Into::into).collect())
        .map_err(|error: ProfileError| BindingError::invalid(format!("{error}")))
}

pub(crate) fn segments(profile: &BasalProfile) -> Vec<BasalSegment> {
    profile.segments().iter().map(Into::into).collect()
}

macro_rules! bounds {
    (
        $bounds:ident, $tier:ident, $raw:ty, $quantity:ty,
        $to:expr, $from:expr,
        $allows:ident, $floor:ident, $nearest:ident, $values:ident $(,)?
    ) => {
        /// One step size, applying from `from` upwards.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
        pub struct $tier {
            pub from: $raw,
            pub step: $raw,
        }

        /// What the device accepts, in the unit this record is named for.
        #[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
        pub struct $bounds {
            pub min: $raw,
            pub max: $raw,
            pub tiers: Vec<$tier>,
        }

        // `Bounds<T>` and its unit are both foreign types, so these are
        // inherent methods rather than `From` impls.
        impl $bounds {
            #[allow(clippy::redundant_closure_call)]
            pub fn from_shared(bounds: &Bounds<$quantity>) -> Self {
                Self {
                    min: $to(bounds.min),
                    max: $to(bounds.max),
                    tiers: bounds
                        .tiers
                        .iter()
                        .map(|tier| $tier {
                            from: $to(tier.from),
                            step: $to(tier.step),
                        })
                        .collect(),
                }
            }

            #[allow(clippy::redundant_closure_call)]
            pub fn to_shared(&self) -> Bounds<$quantity> {
                Bounds {
                    min: $from(self.min),
                    max: $from(self.max),
                    tiers: self
                        .tiers
                        .iter()
                        .map(|tier| Tier {
                            from: $from(tier.from),
                            step: $from(tier.step),
                        })
                        .collect(),
                }
            }
        }

        /// `true` when the device accepts exactly this value.
        #[uniffi::export]
        pub fn $allows(bounds: &$bounds, value: $raw) -> bool {
            #[allow(clippy::redundant_closure_call)]
            bounds.to_shared().allows($from(value))
        }

        /// The greatest accepted value at or below `value` (Loop and Trio
        /// round this way).
        #[uniffi::export]
        pub fn $floor(bounds: &$bounds, value: $raw) -> Option<$raw> {
            #[allow(clippy::redundant_closure_call)]
            bounds.to_shared().floor($from(value)).map($to)
        }

        /// The accepted value closest to `value` (AAPS rounds this way).
        #[uniffi::export]
        pub fn $nearest(bounds: &$bounds, value: $raw) -> Option<$raw> {
            #[allow(clippy::redundant_closure_call)]
            bounds.to_shared().nearest($from(value)).map($to)
        }

        /// Every accepted value, in order. For pickers.
        #[uniffi::export]
        pub fn $values(bounds: &$bounds) -> Vec<$raw> {
            bounds.to_shared().values().into_iter().map($to).collect()
        }
    };
}

bounds!(
    BoundsInsulin,
    TierInsulin,
    u32,
    InsulinAmount,
    |value: InsulinAmount| value.milliunits(),
    InsulinAmount::from_milliunits,
    bounds_insulin_allows,
    bounds_insulin_floor,
    bounds_insulin_nearest,
    bounds_insulin_values,
);

bounds!(
    BoundsBasalRate,
    TierBasalRate,
    u32,
    BasalRate,
    |value: BasalRate| value.milliunits_per_hour(),
    BasalRate::from_milliunits_per_hour,
    bounds_basal_rate_allows,
    bounds_basal_rate_floor,
    bounds_basal_rate_nearest,
    bounds_basal_rate_values,
);

bounds!(
    BoundsPercent,
    TierPercent,
    u16,
    Percent,
    |value: Percent| value.get(),
    Percent::new,
    bounds_percent_allows,
    bounds_percent_floor,
    bounds_percent_nearest,
    bounds_percent_values,
);

bounds!(
    BoundsDuration,
    TierDuration,
    u64,
    Duration,
    |value: Duration| value.as_secs(),
    Duration::from_secs,
    bounds_duration_allows,
    bounds_duration_floor,
    bounds_duration_nearest,
    bounds_duration_values,
);
