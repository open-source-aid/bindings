//! Validating a stored session a host was handed from somewhere else.
//!
//! Some drivers cannot pair a device: `pump-ypso` is one, and the only way
//! its engine reaches `EngineState::Paired` is by loading a session blob a
//! host has already written to `Slot::Session`. Both the store and the
//! keystore belong to the host, so seeding that slot needs nothing from this
//! crate — but *checking* the blob first does, and a host must not grow a
//! second parser for a secret format.
//!
//! So this module exposes the driver's own validator and nothing else. It is
//! pure: no BLE, no state, no storage, and it does not write anything
//! anywhere. The host decides what to do with the answer.
//!
//! Nothing here returns a key or a password, and no error variant quotes any
//! part of the blob — a `SessionBlobError` is safe to show a user and safe to
//! log. The blob itself is not: it carries a 32-byte session key and the
//! device's authorisation password, and it crosses the ABI as a byte array
//! the platform owns (see [`crate::host::secret`]).

use thiserror::Error;

/// What a host may learn about a session blob without being handed its
/// secrets: which device it is for, and what kind of record it is.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct SessionSummary {
    /// The device's BLE address, six bytes.
    pub address: Vec<u8>,
    /// The device serial the blob claims. Checked against the pump itself at
    /// connect, by the driver.
    pub serial: u32,
    /// The record version. Which values are accepted is the driver's to say.
    pub version: u8,
    /// True for a record with no session key (a keyless variant).
    pub keyless: bool,
    /// The leading byte of each credential code — which probe the driver uses
    /// to prove the authorisation password took. Not a secret; the password
    /// it belongs to is not here and never crosses this boundary.
    pub credential_codes: Vec<u8>,
}

/// Why a session blob was refused.
///
/// Deliberately free of any part of the blob, and of any offset into it.
#[derive(Debug, Clone, PartialEq, Eq, Error, uniffi::Error)]
#[non_exhaustive]
pub enum SessionBlobError {
    /// Not a session blob at all: the magic does not match.
    #[error("not a session blob")]
    NotASession,

    /// Shorter than the record's fixed part.
    #[error("session blob is truncated")]
    Truncated,

    /// The checksum over the body does not match its trailer. Corrupt, or
    /// tampered with.
    #[error("session blob failed its checksum")]
    BadChecksum,

    /// A record version this build does not accept.
    #[error("session blob version {version} is not supported by this build")]
    UnsupportedVersion { version: u8 },

    /// The credential list is not well formed.
    #[error("session blob has a malformed credential list")]
    MalformedCredentials,

    /// A field is present but out of range.
    #[error("session blob has a malformed field")]
    MalformedBody,

    /// No driver in this build can read this kind of blob.
    #[error("no driver {driver_id:?} in this build can validate a session blob")]
    UnknownDriver { driver_id: String },
}

/// Validate a YpsoPump session blob and say what it identifies.
///
/// Calls `pump_ypso::inspect_session`, the same parser the engine loads
/// through, so what passes here is what the engine will accept. It is the
/// host's answer to "is this a session, and whose?" before it wraps the bytes
/// and writes them to `Slot::Session`.
#[cfg(feature = "driver-ypso")]
#[uniffi::export]
pub fn ypso_inspect_session(blob: Vec<u8>) -> Result<SessionSummary, SessionBlobError> {
    pump_ypso::inspect_session(&blob)
        .map(|summary| SessionSummary {
            address: summary.mac.to_vec(),
            serial: summary.serial,
            version: summary.version,
            keyless: summary.keyless,
            credential_codes: summary.credential_codes,
        })
        .map_err(Into::into)
}

/// Without the driver, the entry point still exists and still answers — it
/// just says which driver it would have needed.
#[cfg(not(feature = "driver-ypso"))]
#[uniffi::export]
pub fn ypso_inspect_session(_blob: Vec<u8>) -> Result<SessionSummary, SessionBlobError> {
    Err(SessionBlobError::UnknownDriver {
        driver_id: "ypso".to_owned(),
    })
}

#[cfg(feature = "driver-ypso")]
impl From<pump_ypso::SessionError> for SessionBlobError {
    fn from(error: pump_ypso::SessionError) -> Self {
        use pump_ypso::SessionError as E;
        match error {
            E::NotASession => Self::NotASession,
            E::Truncated => Self::Truncated,
            E::BadChecksum => Self::BadChecksum,
            E::UnsupportedVersion(version) => Self::UnsupportedVersion { version },
            E::MalformedCredentials => Self::MalformedCredentials,
            E::MalformedBody => Self::MalformedBody,
            // The driver's enum is `#[non_exhaustive]`: a reason added there
            // and not mapped here is still a refusal, never an accept.
            _ => Self::MalformedBody,
        }
    }
}
