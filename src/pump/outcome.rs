//! Every submitted command finishes with exactly one `Outcome` (E5).
//!
//! The difference between `Refused` and `Unknown` is the one an app must not
//! blur: `Refused` means nothing was sent, so it is safe to change the
//! request and submit again; `Unknown` means something was sent and the
//! result is not known, so the app marks delivery uncertain and never retries
//! automatically (guideline 04).

use pump_shared::{
    Approximation as PsApproximation, Failure as PsFailure, Outcome as PsOutcome,
    Refusal as PsRefusal, Report as PsReport,
};

use crate::pump::history::HistoryEntry;
use crate::pump::status::PumpStatus;
use crate::pump::units::{self, BasalSegment};

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum Refusal {
    /// The feature is not compiled into this build (for example `delivery`).
    NotBuilt,
    NotPaired,
    NotRunning,
    NotSuspended,
    OutOfLimits,
    Busy,
    /// The device cannot do it at all.
    Unsupported,
    Unrecognized,
}

impl From<PsRefusal> for Refusal {
    fn from(refusal: PsRefusal) -> Self {
        match refusal {
            PsRefusal::NotBuilt => Self::NotBuilt,
            PsRefusal::NotPaired => Self::NotPaired,
            PsRefusal::NotRunning => Self::NotRunning,
            PsRefusal::NotSuspended => Self::NotSuspended,
            PsRefusal::OutOfLimits => Self::OutOfLimits,
            PsRefusal::Busy => Self::Busy,
            PsRefusal::Unsupported => Self::Unsupported,
            _ => Self::Unrecognized,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum Failure {
    KeyRejected,
    Status { code: u8 },
    Host,
    Disconnected,
    Protocol,
    Unrecognized,
}

impl From<PsFailure> for Failure {
    fn from(failure: PsFailure) -> Self {
        match failure {
            PsFailure::KeyRejected => Self::KeyRejected,
            PsFailure::Status(code) => Self::Status { code },
            PsFailure::Host => Self::Host,
            PsFailure::Disconnected => Self::Disconnected,
            PsFailure::Protocol => Self::Protocol,
            _ => Self::Unrecognized,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum Approximation {
    Bolus {
        requested_milliunits: u32,
        delivered_milliunits: u32,
        seconds: u64,
    },
    Unrecognized,
}

impl From<PsApproximation> for Approximation {
    fn from(approximation: PsApproximation) -> Self {
        match approximation {
            PsApproximation::Bolus {
                requested,
                delivered,
                duration,
            } => Self::Bolus {
                requested_milliunits: requested.milliunits(),
                delivered_milliunits: delivered.milliunits(),
                seconds: duration.as_secs(),
            },
            _ => Self::Unrecognized,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum Report {
    Done,
    Status { status: PumpStatus },
    Clock { local_seconds: u64 },
    Delivered { milliunits: u32 },
    Profile { segments: Vec<BasalSegment> },
    History { entries: Vec<HistoryEntry> },
    Unrecognized { detail: String },
}

impl From<PsReport> for Report {
    fn from(report: PsReport) -> Self {
        match report {
            PsReport::Done => Self::Done,
            PsReport::Status(status) => Self::Status {
                status: status.into(),
            },
            PsReport::Clock(clock) => Self::Clock {
                local_seconds: clock.local_seconds(),
            },
            PsReport::Delivered(amount) => Self::Delivered {
                milliunits: amount.milliunits(),
            },
            PsReport::Profile(profile) => Self::Profile {
                segments: units::segments(&profile),
            },
            PsReport::History(entries) => Self::History {
                entries: entries.into_iter().map(Into::into).collect(),
            },
            other => Self::Unrecognized {
                detail: format!("{other:?}"),
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum Outcome {
    /// Done exactly as requested.
    Completed {
        report: Report,
    },
    /// Done, but not exactly. Record what was achieved, not what was asked.
    Approximated {
        approximation: Approximation,
    },
    /// Nothing was sent to the device.
    Refused {
        refusal: Refusal,
    },
    /// Something was sent; the result is not known. Never retry automatically.
    Unknown,
    /// Failed in a way known not to have taken effect.
    Failed {
        failure: Failure,
    },
    Unrecognized {
        detail: String,
    },
}

impl From<PsOutcome> for Outcome {
    fn from(outcome: PsOutcome) -> Self {
        match outcome {
            PsOutcome::Completed(report) => Self::Completed {
                report: report.into(),
            },
            PsOutcome::Approximated(approximation) => Self::Approximated {
                approximation: approximation.into(),
            },
            PsOutcome::Refused(refusal) => Self::Refused {
                refusal: refusal.into(),
            },
            PsOutcome::Unknown => Self::Unknown,
            PsOutcome::Failed(failure) => Self::Failed {
                failure: failure.into(),
            },
            other => Self::Unrecognized {
                detail: format!("{other:?}"),
            },
        }
    }
}
