//! The therapy state machine's state, pump status, limits and capabilities.

use pump_shared::{
    Basal as PsBasal, Bolus as PsBolus, Capabilities as PsCapabilities, DeviceInfo as PsDeviceInfo,
    Extended as PsExtended, Limits as PsLimits, Nominal as PsNominal,
    NominalCapabilities as PsNominalCapabilities, NominalLimits as PsNominalLimits,
    PumpMode as PsPumpMode, PumpStatus as PsPumpStatus, TherapyState as PsTherapyState,
};

use crate::pump::history::Alarm;
use crate::pump::units::TempBasalRate;
use crate::pump::units::{BoundsBasalRate, BoundsDuration, BoundsInsulin, BoundsPercent};
use crate::pump::units::{
    NominalBoundsBasalRate, NominalBoundsDuration, NominalBoundsInsulin, NominalBoundsPercent,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum PumpMode {
    Unknown,
    Off,
    Suspended,
    Running,
    Unrecognized,
}

impl From<PsPumpMode> for PumpMode {
    fn from(mode: PsPumpMode) -> Self {
        match mode {
            PsPumpMode::Unknown => Self::Unknown,
            PsPumpMode::Off => Self::Off,
            PsPumpMode::Suspended => Self::Suspended,
            PsPumpMode::Running => Self::Running,
            _ => Self::Unrecognized,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum Basal {
    Unknown,
    Scheduled,
    Temporary { rate: TempBasalRate, seconds: u64 },
    Unrecognized,
}

impl From<PsBasal> for Basal {
    fn from(basal: PsBasal) -> Self {
        match basal {
            PsBasal::Unknown => Self::Unknown,
            PsBasal::Scheduled => Self::Scheduled,
            PsBasal::Temporary { rate, duration } => Self::Temporary {
                rate: rate.into(),
                seconds: duration.as_secs(),
            },
            _ => Self::Unrecognized,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum Bolus {
    Unknown,
    Idle,
    Immediate {
        requested_milliunits: u32,
        delivered_milliunits: u32,
    },
    Unrecognized,
}

impl From<PsBolus> for Bolus {
    fn from(bolus: PsBolus) -> Self {
        match bolus {
            PsBolus::Unknown => Self::Unknown,
            PsBolus::Idle => Self::Idle,
            PsBolus::Immediate {
                requested,
                delivered,
            } => Self::Immediate {
                requested_milliunits: requested.milliunits(),
                delivered_milliunits: delivered.milliunits(),
            },
            _ => Self::Unrecognized,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum Extended {
    Unknown,
    Idle,
    Running { milliunits: u32, seconds: u64 },
    Unrecognized,
}

impl From<PsExtended> for Extended {
    fn from(extended: PsExtended) -> Self {
        match extended {
            PsExtended::Unknown => Self::Unknown,
            PsExtended::Idle => Self::Idle,
            PsExtended::Running { amount, duration } => Self::Running {
                milliunits: amount.milliunits(),
                seconds: duration.as_secs(),
            },
            _ => Self::Unrecognized,
        }
    }
}

/// What the pump is doing. `Unknown` in any field means the engine has not
/// observed it yet, not that nothing is running.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct TherapyState {
    pub mode: PumpMode,
    pub basal: Basal,
    pub bolus: Bolus,
    pub extended: Extended,
}

impl From<PsTherapyState> for TherapyState {
    fn from(state: PsTherapyState) -> Self {
        Self {
            mode: state.mode.into(),
            basal: state.basal.into(),
            bolus: state.bolus.into(),
            extended: state.extended.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct PumpStatus {
    pub therapy: TherapyState,
    pub battery_percent: Option<u16>,
    pub reservoir_milliunits: Option<u32>,
    pub basal_milliunits_per_hour: Option<u32>,
    pub clock_local_seconds: Option<u64>,
    /// Something may have been delivered and the engine cannot tell. The app
    /// reconciles from status and history and never retries automatically.
    pub delivery_uncertain: bool,
    pub alarms: Vec<Alarm>,
}

impl From<PsPumpStatus> for PumpStatus {
    fn from(status: PsPumpStatus) -> Self {
        Self {
            therapy: status.therapy.into(),
            battery_percent: status.battery.map(|percent| percent.get()),
            reservoir_milliunits: status.reservoir.map(|amount| amount.milliunits()),
            basal_milliunits_per_hour: status.basal_rate.map(|rate| rate.milliunits_per_hour()),
            clock_local_seconds: status.clock.map(|clock| clock.local_seconds()),
            delivery_uncertain: status.delivery_uncertain,
            alarms: status.alarms.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct Capabilities {
    pub reservoir_capacity_milliunits: Option<u32>,
    pub max_profile_segments: u8,
    pub min_segment_seconds: u64,
    pub history: bool,
    pub daily_totals: bool,
    pub patch: bool,
}

impl Capabilities {
    pub fn from_shared(capabilities: &PsCapabilities) -> Self {
        Self {
            reservoir_capacity_milliunits: capabilities
                .reservoir_capacity
                .map(|amount| amount.milliunits()),
            max_profile_segments: capabilities.max_profile_segments,
            min_segment_seconds: capabilities.min_segment_duration.as_secs(),
            history: capabilities.history,
            daily_totals: capabilities.daily_totals,
            patch: capabilities.patch,
        }
    }
}

/// What the device accepts. The adapter rounds a request into these bounds
/// before submitting it (`bounds_*_floor` or `bounds_*_nearest`).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct Limits {
    pub bolus: BoundsInsulin,
    pub basal: BoundsBasalRate,
    pub temp_basal_percent: Option<BoundsPercent>,
    pub temp_basal_absolute: Option<BoundsBasalRate>,
    pub temp_basal_duration: BoundsDuration,
    pub extended_bolus_duration: Option<BoundsDuration>,
}

impl Limits {
    pub fn from_shared(limits: &PsLimits) -> Self {
        Self {
            bolus: BoundsInsulin::from_shared(&limits.bolus),
            basal: BoundsBasalRate::from_shared(&limits.basal),
            temp_basal_percent: limits
                .temp_basal_percent
                .as_ref()
                .map(BoundsPercent::from_shared),
            temp_basal_absolute: limits
                .temp_basal_absolute
                .as_ref()
                .map(BoundsBasalRate::from_shared),
            temp_basal_duration: BoundsDuration::from_shared(&limits.temp_basal_duration),
            extended_bolus_duration: limits
                .extended_bolus_duration
                .as_ref()
                .map(BoundsDuration::from_shared),
        }
    }

    pub fn to_shared(&self) -> PsLimits {
        PsLimits {
            bolus: self.bolus.to_shared(),
            basal: self.basal.to_shared(),
            temp_basal_percent: self
                .temp_basal_percent
                .as_ref()
                .map(BoundsPercent::to_shared),
            temp_basal_absolute: self
                .temp_basal_absolute
                .as_ref()
                .map(BoundsBasalRate::to_shared),
            temp_basal_duration: self.temp_basal_duration.to_shared(),
            extended_bolus_duration: self
                .extended_bolus_duration
                .as_ref()
                .map(BoundsDuration::to_shared),
        }
    }
}

/// What the device said about itself after a connect.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct DeviceInfo {
    pub serial: String,
    pub firmware: String,
    pub variant: String,
    pub limits: Limits,
    pub capabilities: Capabilities,
}

impl From<PsDeviceInfo> for DeviceInfo {
    fn from(info: PsDeviceInfo) -> Self {
        Self {
            serial: info.serial,
            firmware: info.firmware,
            variant: info.variant.to_owned(),
            limits: Limits::from_shared(&info.limits),
            capabilities: Capabilities::from_shared(&info.capabilities),
        }
    }
}

/// Does the device accept this temp basal? Delegates to `Limits`.
#[uniffi::export]
pub fn limits_allow_temp_basal(limits: &Limits, rate: TempBasalRate, seconds: u64) -> bool {
    limits
        .to_shared()
        .allows_temp_basal(rate.into(), core::time::Duration::from_secs(seconds))
}

/// Does the device accept this extended bolus? Delegates to `Limits`.
#[uniffi::export]
pub fn limits_allow_extended_bolus(limits: &Limits, milliunits: u32, seconds: u64) -> bool {
    limits.to_shared().allows_extended_bolus(
        pump_shared::InsulinAmount::from_milliunits(milliunits),
        core::time::Duration::from_secs(seconds),
    )
}

/// What a device **family** is built for, declared by the driver at compile
/// time and readable before any device is connected — the figures a setup
/// screen needs.
///
/// Not [`Limits`]: see [`NominalLimits`] for what that distinction costs if
/// it is ignored.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct NominalLimits {
    /// Bolus amounts. `max` is the family ceiling; the user's own maximum
    /// bolus lives on the device and is lower.
    pub bolus: Option<NominalBoundsInsulin>,
    pub basal: Option<NominalBoundsBasalRate>,
    pub temp_basal_percent: Option<NominalBoundsPercent>,
    pub temp_basal_absolute: Option<NominalBoundsBasalRate>,
    pub temp_basal_duration: Option<NominalBoundsDuration>,
    pub extended_bolus_duration: Option<NominalBoundsDuration>,
}

impl NominalLimits {
    pub fn from_shared(limits: &PsNominalLimits) -> Self {
        Self {
            bolus: limits.bolus.as_ref().map(NominalBoundsInsulin::from_shared),
            basal: limits
                .basal
                .as_ref()
                .map(NominalBoundsBasalRate::from_shared),
            temp_basal_percent: limits
                .temp_basal_percent
                .as_ref()
                .map(NominalBoundsPercent::from_shared),
            temp_basal_absolute: limits
                .temp_basal_absolute
                .as_ref()
                .map(NominalBoundsBasalRate::from_shared),
            temp_basal_duration: limits
                .temp_basal_duration
                .as_ref()
                .map(NominalBoundsDuration::from_shared),
            extended_bolus_duration: limits
                .extended_bolus_duration
                .as_ref()
                .map(NominalBoundsDuration::from_shared),
        }
    }
}

/// The family counterpart of [`Capabilities`]. A measurement the driver
/// cannot evidence is `None`; the three flags are facts about the family.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct NominalCapabilities {
    /// A full cartridge, not a fill level.
    pub reservoir_capacity_milliunits: Option<u32>,
    pub max_profile_segments: Option<u8>,
    pub min_segment_seconds: Option<u64>,
    pub history: bool,
    pub daily_totals: bool,
    pub patch: bool,
}

impl NominalCapabilities {
    pub fn from_shared(capabilities: &PsNominalCapabilities) -> Self {
        Self {
            reservoir_capacity_milliunits: capabilities
                .reservoir_capacity
                .map(|amount| amount.milliunits()),
            max_profile_segments: capabilities.max_profile_segments,
            min_segment_seconds: capabilities
                .min_segment_duration
                .map(|duration| duration.as_secs()),
            history: capabilities.history,
            daily_totals: capabilities.daily_totals,
            patch: capabilities.patch,
        }
    }
}

/// Everything a driver says about its family before a device exists, carried
/// on `DriverInfo.nominal` and, per model, on `Variant.nominal`.
///
/// **Nominal is not actual.** A pump is narrower than its family, and only
/// the pump can say by how much, in [`DeviceInfo`]. Draw a picker from this;
/// round and validate a dose with [`Limits`] from the device, or do not round
/// at all.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct Nominal {
    pub limits: NominalLimits,
    pub capabilities: NominalCapabilities,
}

impl From<&PsNominal> for Nominal {
    fn from(nominal: &PsNominal) -> Self {
        Self {
            limits: NominalLimits::from_shared(&nominal.limits),
            capabilities: NominalCapabilities::from_shared(&nominal.capabilities),
        }
    }
}
