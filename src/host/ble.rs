//! The BLE half of the host contract: `Uuid`, `Address`, `BleOp`, `BleResult`
//! and `Notification`.
//!
//! `BleOp` is exhaustive in the Rust contract (guideline 04), so it is
//! exhaustive here too: adding a variant is a breaking change every host must
//! handle. `BleResult` is not, so it has an `Unrecognized` arm.

use pump_shared::{Address, BleOp as PsBleOp, BleResult as PsBleResult, Uuid};
use pump_shared::{Notification as PsNotification, NotifyKind as PsNotifyKind};

use crate::error::{self, BindingError};

// A UUID and a BLE address cross as their raw bytes, length-checked on the
// way in. Neither has a parser in the shared crate, and the bindings add no
// logic, so neither gets one here.
uniffi::custom_type!(Uuid, Vec<u8>, {
    remote,
    lower: |uuid| uuid.as_bytes().to_vec(),
    try_lift: |bytes| Ok(error::uuid(&bytes)?),
});

uniffi::custom_type!(Address, Vec<u8>, {
    remote,
    lower: |address| address.to_bytes().to_vec(),
    try_lift: |bytes| Ok(error::address(&bytes)?),
});

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum NotifyKind {
    Notification,
    Indication,
}

impl From<PsNotifyKind> for NotifyKind {
    fn from(kind: PsNotifyKind) -> Self {
        match kind {
            PsNotifyKind::Notification => Self::Notification,
            PsNotifyKind::Indication => Self::Indication,
        }
    }
}

impl From<NotifyKind> for PsNotifyKind {
    fn from(kind: NotifyKind) -> Self {
        match kind {
            NotifyKind::Notification => Self::Notification,
            NotifyKind::Indication => Self::Indication,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct Notification {
    pub service: Uuid,
    pub characteristic: Uuid,
    pub data: Vec<u8>,
}

impl From<PsNotification> for Notification {
    fn from(notification: PsNotification) -> Self {
        Self {
            service: notification.service,
            characteristic: notification.characteristic,
            data: notification.data,
        }
    }
}

impl From<Notification> for PsNotification {
    fn from(notification: Notification) -> Self {
        Self {
            service: notification.service,
            characteristic: notification.characteristic,
            data: notification.data,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum BleOp {
    Connect {
        address: Address,
    },
    Disconnect,
    Read {
        service: Uuid,
        characteristic: Uuid,
    },
    Write {
        service: Uuid,
        characteristic: Uuid,
        data: Vec<u8>,
    },
    Subscribe {
        service: Uuid,
        characteristic: Uuid,
        kind: NotifyKind,
    },
    Unsubscribe {
        service: Uuid,
        characteristic: Uuid,
    },
    RequestFastInterval,
}

impl From<PsBleOp> for BleOp {
    fn from(op: PsBleOp) -> Self {
        match op {
            PsBleOp::Connect { address } => Self::Connect { address },
            PsBleOp::Disconnect => Self::Disconnect,
            PsBleOp::Read {
                service,
                characteristic,
            } => Self::Read {
                service,
                characteristic,
            },
            PsBleOp::Write {
                service,
                characteristic,
                data,
            } => Self::Write {
                service,
                characteristic,
                data,
            },
            PsBleOp::Subscribe {
                service,
                characteristic,
                kind,
            } => Self::Subscribe {
                service,
                characteristic,
                kind: kind.into(),
            },
            PsBleOp::Unsubscribe {
                service,
                characteristic,
            } => Self::Unsubscribe {
                service,
                characteristic,
            },
            PsBleOp::RequestFastInterval => Self::RequestFastInterval,
        }
    }
}

impl From<BleOp> for PsBleOp {
    fn from(op: BleOp) -> Self {
        match op {
            BleOp::Connect { address } => Self::Connect { address },
            BleOp::Disconnect => Self::Disconnect,
            BleOp::Read {
                service,
                characteristic,
            } => Self::Read {
                service,
                characteristic,
            },
            BleOp::Write {
                service,
                characteristic,
                data,
            } => Self::Write {
                service,
                characteristic,
                data,
            },
            BleOp::Subscribe {
                service,
                characteristic,
                kind,
            } => Self::Subscribe {
                service,
                characteristic,
                kind: kind.into(),
            },
            BleOp::Unsubscribe {
                service,
                characteristic,
            } => Self::Unsubscribe {
                service,
                characteristic,
            },
            BleOp::RequestFastInterval => Self::RequestFastInterval,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum BleResult {
    Done,
    Data {
        bytes: Vec<u8>,
    },
    Status {
        code: u8,
    },
    Timeout,
    Disconnected,
    Failed,
    /// The shared contract grew a `BleResult` this binding does not map.
    Unrecognized {
        detail: String,
    },
}

impl From<PsBleResult> for BleResult {
    fn from(result: PsBleResult) -> Self {
        match result {
            PsBleResult::Done => Self::Done,
            PsBleResult::Data(bytes) => Self::Data { bytes },
            PsBleResult::Status(code) => Self::Status { code },
            PsBleResult::Timeout => Self::Timeout,
            PsBleResult::Disconnected => Self::Disconnected,
            PsBleResult::Failed => Self::Failed,
            other => Self::Unrecognized {
                detail: format!("{other:?}"),
            },
        }
    }
}

impl TryFrom<BleResult> for PsBleResult {
    type Error = BindingError;

    fn try_from(result: BleResult) -> Result<Self, Self::Error> {
        Ok(match result {
            BleResult::Done => Self::Done,
            BleResult::Data { bytes } => Self::Data(bytes),
            BleResult::Status { code } => Self::Status(code),
            BleResult::Timeout => Self::Timeout,
            BleResult::Disconnected => Self::Disconnected,
            BleResult::Failed => Self::Failed,
            BleResult::Unrecognized { detail } => {
                return Err(BindingError::invalid(format!(
                    "BleResult::Unrecognized({detail}) cannot be given back to an engine"
                )))
            }
        })
    }
}
