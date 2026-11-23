//! `Action`, `Completion`, `Slot` and `Class`: what the host loop carries.
//!
//! `Action` and `Completion` are exhaustive in the Rust contract, so they are
//! exhaustive here. `Slot` and `Class` are not, so each has an `Unrecognized`
//! arm; a host that sees one has a driver newer than its bindings.

use pump_shared::{Action as PsAction, Class as PsClass, Completion as PsCompletion};
use pump_shared::{KeyRef, OpId, Slot as PsSlot};

use crate::error::{self, BindingError};
use crate::host::ble::BleOp;
use crate::host::ble::BleResult;
use crate::host::secret::SecretBytes;

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum Slot {
    Session,
    PendingSession,
    Counters,
    NonceState,
    Driver { index: u8 },
    Unrecognized { detail: String },
}

impl From<PsSlot> for Slot {
    fn from(slot: PsSlot) -> Self {
        match slot {
            PsSlot::Session => Self::Session,
            PsSlot::PendingSession => Self::PendingSession,
            PsSlot::Counters => Self::Counters,
            PsSlot::NonceState => Self::NonceState,
            PsSlot::Driver(index) => Self::Driver { index },
            other => Self::Unrecognized {
                detail: format!("{other:?}"),
            },
        }
    }
}

impl TryFrom<Slot> for PsSlot {
    type Error = BindingError;

    fn try_from(slot: Slot) -> Result<Self, Self::Error> {
        Ok(match slot {
            Slot::Session => Self::Session,
            Slot::PendingSession => Self::PendingSession,
            Slot::Counters => Self::Counters,
            Slot::NonceState => Self::NonceState,
            Slot::Driver { index } => Self::Driver(index),
            Slot::Unrecognized { detail } => {
                return Err(BindingError::invalid(format!("unknown slot: {detail}")))
            }
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum Class {
    /// Keystore or keychain. The host MUST NOT write it to plain storage.
    Secret,
    /// Written before the value is used on the wire (E7).
    Monotonic,
    Plain,
    Unrecognized {
        detail: String,
    },
}

impl From<PsClass> for Class {
    fn from(class: PsClass) -> Self {
        match class {
            PsClass::Secret => Self::Secret,
            PsClass::Monotonic => Self::Monotonic,
            PsClass::Plain => Self::Plain,
            other => Self::Unrecognized {
                detail: format!("{other:?}"),
            },
        }
    }
}

impl TryFrom<Class> for PsClass {
    type Error = BindingError;

    fn try_from(class: Class) -> Result<Self, Self::Error> {
        Ok(match class {
            Class::Secret => Self::Secret,
            Class::Monotonic => Self::Monotonic,
            Class::Plain => Self::Plain,
            Class::Unrecognized { detail } => {
                return Err(BindingError::invalid(format!("unknown class: {detail}")))
            }
        })
    }
}

/// One thing the host must do. At most one is outstanding at a time (E1), and
/// its `id` is the id the matching `Completion` is handed back with.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum Action {
    Ble {
        id: u32,
        op: BleOp,
    },
    Store {
        id: u32,
        slot: Slot,
        class: Class,
        bytes: Vec<u8>,
    },
    Load {
        id: u32,
        slot: Slot,
    },
    Erase {
        id: u32,
        slot: Slot,
    },
    Wrap {
        id: u32,
        plaintext: SecretBytes,
    },
    Unwrap {
        id: u32,
        wrapped: Vec<u8>,
    },
    KeyAgree {
        id: u32,
        /// Name of a key the host holds.
        key: String,
        peer_public: Vec<u8>,
    },
    Random {
        id: u32,
        len: u32,
    },
}

impl Action {
    /// The id the completion for this action must carry.
    pub fn id(&self) -> u32 {
        match self {
            Self::Ble { id, .. }
            | Self::Store { id, .. }
            | Self::Load { id, .. }
            | Self::Erase { id, .. }
            | Self::Wrap { id, .. }
            | Self::Unwrap { id, .. }
            | Self::KeyAgree { id, .. }
            | Self::Random { id, .. } => *id,
        }
    }
}

/// The id the completion for this action must carry (E1). The host loop
/// needs it before it performs the action, so it is a function rather than a
/// field repeated in every variant.
#[uniffi::export]
pub fn action_id(action: &Action) -> u32 {
    action.id()
}

impl From<PsAction> for Action {
    fn from(action: PsAction) -> Self {
        match action {
            PsAction::Ble { id, op } => Self::Ble {
                id: id.get(),
                op: op.into(),
            },
            PsAction::Store {
                id,
                slot,
                class,
                bytes,
            } => Self::Store {
                id: id.get(),
                slot: slot.into(),
                class: class.into(),
                bytes,
            },
            PsAction::Load { id, slot } => Self::Load {
                id: id.get(),
                slot: slot.into(),
            },
            PsAction::Erase { id, slot } => Self::Erase {
                id: id.get(),
                slot: slot.into(),
            },
            PsAction::Wrap { id, plaintext } => Self::Wrap {
                id: id.get(),
                plaintext: plaintext.into(),
            },
            PsAction::Unwrap { id, wrapped } => Self::Unwrap {
                id: id.get(),
                wrapped,
            },
            PsAction::KeyAgree {
                id,
                key,
                peer_public,
            } => Self::KeyAgree {
                id: id.get(),
                key: key.as_str().to_owned(),
                peer_public: peer_public.to_vec(),
            },
            PsAction::Random { id, len } => Self::Random {
                id: id.get(),
                len: u32::try_from(len).unwrap_or(u32::MAX),
            },
        }
    }
}

impl TryFrom<Action> for PsAction {
    type Error = BindingError;

    fn try_from(action: Action) -> Result<Self, Self::Error> {
        Ok(match action {
            Action::Ble { id, op } => Self::Ble {
                id: OpId::new(id),
                op: op.into(),
            },
            Action::Store {
                id,
                slot,
                class,
                bytes,
            } => Self::Store {
                id: OpId::new(id),
                slot: slot.try_into()?,
                class: class.try_into()?,
                bytes,
            },
            Action::Load { id, slot } => Self::Load {
                id: OpId::new(id),
                slot: slot.try_into()?,
            },
            Action::Erase { id, slot } => Self::Erase {
                id: OpId::new(id),
                slot: slot.try_into()?,
            },
            Action::Wrap { id, plaintext } => Self::Wrap {
                id: OpId::new(id),
                plaintext: plaintext.into(),
            },
            Action::Unwrap { id, wrapped } => Self::Unwrap {
                id: OpId::new(id),
                wrapped,
            },
            Action::KeyAgree {
                id,
                key,
                peer_public,
            } => Self::KeyAgree {
                id: OpId::new(id),
                key: KeyRef::new(key),
                peer_public: error::public_key(&peer_public)?,
            },
            Action::Random { id, len } => Self::Random {
                id: OpId::new(id),
                len: len as usize,
            },
        })
    }
}

/// What the host hands back for exactly one `Action`.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum Completion {
    Ble {
        result: BleResult,
    },
    Stored,
    Loaded {
        bytes: Option<Vec<u8>>,
    },
    Wrapped {
        bytes: Vec<u8>,
    },
    Secret {
        secret: SecretBytes,
    },
    /// The host could not perform the action. Valid for any action.
    Failed,
}

impl From<PsCompletion> for Completion {
    fn from(completion: PsCompletion) -> Self {
        match completion {
            PsCompletion::Ble(result) => Self::Ble {
                result: result.into(),
            },
            PsCompletion::Stored => Self::Stored,
            PsCompletion::Loaded(bytes) => Self::Loaded { bytes },
            PsCompletion::Wrapped(bytes) => Self::Wrapped { bytes },
            PsCompletion::Secret(secret) => Self::Secret {
                secret: secret.into(),
            },
            PsCompletion::Failed => Self::Failed,
        }
    }
}

impl TryFrom<Completion> for PsCompletion {
    type Error = BindingError;

    fn try_from(completion: Completion) -> Result<Self, Self::Error> {
        Ok(match completion {
            Completion::Ble { result } => Self::Ble(result.try_into()?),
            Completion::Stored => Self::Stored,
            Completion::Loaded { bytes } => Self::Loaded(bytes),
            Completion::Wrapped { bytes } => Self::Wrapped(bytes),
            Completion::Secret { secret } => Self::Secret(secret.into()),
            Completion::Failed => Self::Failed,
        })
    }
}
