//! Pump history, as it leaves the engine.
//!
//! History only ever travels engine to host, so these records convert one
//! way. `raw` carries the device's own bytes so an app can keep what it does
//! not understand.

use pump_shared::{
    Alarm as PsAlarm, AlarmKind as PsAlarmKind, BolusKind as PsBolusKind, Component as PsComponent,
    HistoryEntry as PsHistoryEntry, HistoryEvent as PsHistoryEvent, LimitChange as PsLimitChange,
    TempBasalPhase as PsTempBasalPhase,
};

use pump_shared::{Sequence, UtcTime};

use crate::pump::units::TempBasalRate;

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum BolusKind {
    Normal,
    Smb,
    Priming,
    Extended,
    Combined,
    Unrecognized,
}

impl From<PsBolusKind> for BolusKind {
    fn from(kind: PsBolusKind) -> Self {
        match kind {
            PsBolusKind::Normal => Self::Normal,
            PsBolusKind::Smb => Self::Smb,
            PsBolusKind::Priming => Self::Priming,
            PsBolusKind::Extended => Self::Extended,
            PsBolusKind::Combined => Self::Combined,
            _ => Self::Unrecognized,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum TempBasalPhase {
    Started,
    Running,
    Ended,
    Unrecognized,
}

impl From<PsTempBasalPhase> for TempBasalPhase {
    fn from(phase: PsTempBasalPhase) -> Self {
        match phase {
            PsTempBasalPhase::Started => Self::Started,
            PsTempBasalPhase::Running => Self::Running,
            PsTempBasalPhase::Ended => Self::Ended,
            _ => Self::Unrecognized,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum Component {
    Reservoir,
    Cannula,
    Tubing,
    Battery,
    Pump,
    Unrecognized,
}

impl From<PsComponent> for Component {
    fn from(component: PsComponent) -> Self {
        match component {
            PsComponent::Reservoir => Self::Reservoir,
            PsComponent::Cannula => Self::Cannula,
            PsComponent::Tubing => Self::Tubing,
            PsComponent::Battery => Self::Battery,
            PsComponent::Pump => Self::Pump,
            _ => Self::Unrecognized,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum AlarmKind {
    Occlusion,
    ReservoirLow,
    ReservoirEmpty,
    BatteryLow,
    BatteryEmpty,
    AutoOff,
    NoDelivery,
    Other,
    Unrecognized,
}

impl From<PsAlarmKind> for AlarmKind {
    fn from(kind: PsAlarmKind) -> Self {
        match kind {
            PsAlarmKind::Occlusion => Self::Occlusion,
            PsAlarmKind::ReservoirLow => Self::ReservoirLow,
            PsAlarmKind::ReservoirEmpty => Self::ReservoirEmpty,
            PsAlarmKind::BatteryLow => Self::BatteryLow,
            PsAlarmKind::BatteryEmpty => Self::BatteryEmpty,
            PsAlarmKind::AutoOff => Self::AutoOff,
            PsAlarmKind::NoDelivery => Self::NoDelivery,
            PsAlarmKind::Other => Self::Other,
            _ => Self::Unrecognized,
        }
    }
}

/// An alarm the device raised. `code` is the device's own number, kept so an
/// app can show it even when `kind` is `Other`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct Alarm {
    pub kind: AlarmKind,
    pub code: u16,
}

impl From<PsAlarm> for Alarm {
    fn from(alarm: PsAlarm) -> Self {
        Self {
            kind: alarm.kind.into(),
            code: alarm.code,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum LimitChange {
    MaxBolus {
        milliunits: u32,
        previous_milliunits: u32,
    },
    MaxBasal {
        milliunits_per_hour: u32,
        previous_milliunits_per_hour: u32,
    },
    BolusStep {
        milliunits: u32,
    },
    Unrecognized,
}

impl From<PsLimitChange> for LimitChange {
    fn from(change: PsLimitChange) -> Self {
        match change {
            PsLimitChange::MaxBolus { value, previous } => Self::MaxBolus {
                milliunits: value.milliunits(),
                previous_milliunits: previous.milliunits(),
            },
            PsLimitChange::MaxBasal { value, previous } => Self::MaxBasal {
                milliunits_per_hour: value.milliunits_per_hour(),
                previous_milliunits_per_hour: previous.milliunits_per_hour(),
            },
            PsLimitChange::BolusStep { value } => Self::BolusStep {
                milliunits: value.milliunits(),
            },
            _ => Self::Unrecognized,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum HistoryEvent {
    Bolus {
        kind: BolusKind,
        requested_milliunits: u32,
        delivered_milliunits: u32,
        seconds: u64,
        automatic: bool,
        in_progress: bool,
    },
    ExtendedBolusEnded {
        delivered_milliunits: u32,
    },
    TempBasal {
        rate: TempBasalRate,
        seconds: u64,
        phase: TempBasalPhase,
    },
    Suspended,
    Resumed,
    BasalProfileChanged,
    Rewound,
    Replaced {
        component: Component,
    },
    ClockChanged {
        previous_local_seconds: u64,
    },
    Alarm {
        alarm: Alarm,
    },
    AlarmCleared {
        alarm: Alarm,
    },
    DailyTotal {
        day_local_seconds: u64,
        milliunits: u32,
    },
    LimitChanged {
        change: LimitChange,
    },
    /// The driver reported an event this binding does not map.
    Unrecognized {
        detail: String,
    },
}

impl From<PsHistoryEvent> for HistoryEvent {
    fn from(event: PsHistoryEvent) -> Self {
        match event {
            PsHistoryEvent::Bolus {
                kind,
                requested,
                delivered,
                duration,
                automatic,
                in_progress,
            } => Self::Bolus {
                kind: kind.into(),
                requested_milliunits: requested.milliunits(),
                delivered_milliunits: delivered.milliunits(),
                seconds: duration.as_secs(),
                automatic,
                in_progress,
            },
            PsHistoryEvent::ExtendedBolusEnded { delivered } => Self::ExtendedBolusEnded {
                delivered_milliunits: delivered.milliunits(),
            },
            PsHistoryEvent::TempBasal {
                rate,
                duration,
                phase,
            } => Self::TempBasal {
                rate: rate.into(),
                seconds: duration.as_secs(),
                phase: phase.into(),
            },
            PsHistoryEvent::Suspended => Self::Suspended,
            PsHistoryEvent::Resumed => Self::Resumed,
            PsHistoryEvent::BasalProfileChanged => Self::BasalProfileChanged,
            PsHistoryEvent::Rewound => Self::Rewound,
            PsHistoryEvent::Replaced(component) => Self::Replaced {
                component: component.into(),
            },
            PsHistoryEvent::ClockChanged { previous } => Self::ClockChanged {
                previous_local_seconds: previous.local_seconds(),
            },
            PsHistoryEvent::Alarm(alarm) => Self::Alarm {
                alarm: alarm.into(),
            },
            PsHistoryEvent::AlarmCleared(alarm) => Self::AlarmCleared {
                alarm: alarm.into(),
            },
            PsHistoryEvent::DailyTotal { day, amount } => Self::DailyTotal {
                day_local_seconds: day.local_seconds(),
                milliunits: amount.milliunits(),
            },
            PsHistoryEvent::LimitChanged(change) => Self::LimitChanged {
                change: change.into(),
            },
            other => Self::Unrecognized {
                detail: format!("{other:?}"),
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct HistoryEntry {
    /// The device's own record number. With the serial it forms the identity
    /// an app deduplicates on (`sync_id`).
    pub sequence: u32,
    pub unix_millis: u64,
    /// The device's local clock, when it reports one.
    pub pump_local_seconds: Option<u64>,
    pub event: HistoryEvent,
    /// The device's own bytes for this record.
    pub raw: Vec<u8>,
}

impl From<PsHistoryEntry> for HistoryEntry {
    fn from(entry: PsHistoryEntry) -> Self {
        Self {
            sequence: entry.sequence.get(),
            unix_millis: entry.time.as_unix_millis(),
            pump_local_seconds: entry.pump_time.map(|time| time.local_seconds()),
            event: entry.event.into(),
            raw: entry.raw,
        }
    }
}

/// The identity an app deduplicates history on, built the one way the shared
/// crate builds it. The serial comes from `Event::DeviceInfo`; the bindings
/// keep no state, so the caller passes it.
#[uniffi::export]
pub fn history_sync_id(serial: String, sequence: u32) -> String {
    PsHistoryEntry {
        sequence: Sequence::new(sequence),
        time: UtcTime::default(),
        pump_time: None,
        event: PsHistoryEvent::Rewound,
        raw: Vec::new(),
    }
    .sync_id(&serial)
}
