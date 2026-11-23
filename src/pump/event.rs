//! What the engine tells the host, drained with `pollEvent` before every
//! `pollAction` (guideline 04, host loop).

use pump_shared::Event as PsEvent;

use crate::host::driver::EngineState;
use crate::pump::outcome::Outcome;
use crate::pump::status::{DeviceInfo, TherapyState};

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum Event {
    /// A command began executing.
    Started {
        id: u32,
    },
    /// During a bolus.
    Progress {
        id: u32,
        delivered_milliunits: u32,
        requested_milliunits: u32,
    },
    /// Exactly once per submitted command (E5).
    Finished {
        id: u32,
        outcome: Outcome,
    },
    DeviceInfo {
        info: DeviceInfo,
    },
    StateChanged {
        state: EngineState,
    },
    TherapyChanged {
        therapy: TherapyState,
    },
    /// Diagnostic text. Never contains a secret (E10).
    Log {
        level: u8,
        line: String,
    },
    Unrecognized {
        detail: String,
    },
}

impl From<PsEvent> for Event {
    fn from(event: PsEvent) -> Self {
        match event {
            PsEvent::Started { id } => Self::Started { id: id.get() },
            PsEvent::Progress {
                id,
                delivered,
                requested,
            } => Self::Progress {
                id: id.get(),
                delivered_milliunits: delivered.milliunits(),
                requested_milliunits: requested.milliunits(),
            },
            PsEvent::Finished { id, outcome } => Self::Finished {
                id: id.get(),
                outcome: outcome.into(),
            },
            PsEvent::DeviceInfo(info) => Self::DeviceInfo {
                info: (*info).into(),
            },
            PsEvent::StateChanged(state) => Self::StateChanged {
                state: state.into(),
            },
            PsEvent::TherapyChanged(therapy) => Self::TherapyChanged {
                therapy: therapy.into(),
            },
            PsEvent::Log { level, line } => Self::Log { level, line },
            other => Self::Unrecognized {
                detail: format!("{other:?}"),
            },
        }
    }
}
