//! The one error type that crosses the ABI.
//!
//! The bindings add no logic, so this enum carries only two kinds of thing:
//! an error the shared contract already defines (`SubmitError`,
//! `CompletionError`), and a reason the value could not cross the ABI at all.
//! Nothing here panics: a gap in this skeleton surfaces as
//! [`BindingError::NotImplemented`], never as an abort across the FFI
//! boundary.

use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error, uniffi::Error)]
#[non_exhaustive]
pub enum BindingError {
    /// `SubmitError::QueueFull` (E3).
    #[error("command queue is full")]
    QueueFull,

    /// `CompletionError::NotPending` (E1).
    #[error("no pending operation with this id")]
    NotPending,

    /// `CompletionError::Mismatch` (E1).
    #[error("completion does not match the pending operation")]
    Mismatch,

    /// A value arrived from Swift or Kotlin that the Rust contract cannot
    /// represent: a wrong-length address, an unknown id, a variant that only
    /// exists on the FFI side.
    #[error("invalid value: {detail}")]
    Invalid { detail: String },

    /// The Rust contract holds a variant this binding version does not map.
    /// Both sides keep working; the host treats it as an unknown value.
    #[error("not representable across the ABI: {detail}")]
    Unsupported { detail: String },

    /// A deliberate gap in this skeleton. Listed in the README.
    #[error("not implemented yet: {what}")]
    NotImplemented { what: String },
}

impl BindingError {
    pub(crate) fn invalid(detail: impl Into<String>) -> Self {
        Self::Invalid {
            detail: detail.into(),
        }
    }

    pub(crate) fn unsupported(detail: impl Into<String>) -> Self {
        Self::Unsupported {
            detail: detail.into(),
        }
    }

    pub(crate) fn not_implemented(what: impl Into<String>) -> Self {
        Self::NotImplemented { what: what.into() }
    }
}

impl From<pump_shared::SubmitError> for BindingError {
    fn from(error: pump_shared::SubmitError) -> Self {
        match error {
            pump_shared::SubmitError::QueueFull => Self::QueueFull,
            other => Self::unsupported(format!("{other:?}")),
        }
    }
}

impl From<pump_shared::CompletionError> for BindingError {
    fn from(error: pump_shared::CompletionError) -> Self {
        match error {
            pump_shared::CompletionError::NotPending => Self::NotPending,
            pump_shared::CompletionError::Mismatch => Self::Mismatch,
            other => Self::unsupported(format!("{other:?}")),
        }
    }
}

/// Six bytes, the way every BLE address crosses the ABI.
pub fn address(bytes: &[u8]) -> Result<pump_shared::Address, BindingError> {
    <[u8; 6]>::try_from(bytes)
        .map(pump_shared::Address::new)
        .map_err(|_| BindingError::invalid("a BLE address is 6 bytes"))
}

/// Sixteen bytes, the way every UUID crosses the ABI.
pub fn uuid(bytes: &[u8]) -> Result<pump_shared::Uuid, BindingError> {
    <[u8; 16]>::try_from(bytes)
        .map(pump_shared::Uuid::from_bytes)
        .map_err(|_| BindingError::invalid("a UUID is 16 bytes"))
}

/// Thirty-two bytes, the way an X25519 public key crosses the ABI.
pub fn public_key(bytes: &[u8]) -> Result<[u8; 32], BindingError> {
    <[u8; 32]>::try_from(bytes).map_err(|_| BindingError::invalid("a public key is 32 bytes"))
}
