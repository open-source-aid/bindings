//! `Secret` across the ABI (E10, S10).
//!
//! A secret crosses as plain bytes, because that is the only thing the ABI
//! can carry. Everything the Rust side can still do, it still does:
//!
//! * the Rust-side value is a [`zeroize::Zeroizing`] buffer, so it is zeroed
//!   when it is dropped, whichever direction it travelled;
//! * `Debug` prints a length, never a byte (see the test at the bottom);
//! * it is never logged, and `Event::Log` carries no secret.
//!
//! What crossing the ABI costs, and the bindings cannot avoid: the bytes are
//! copied into the FFI buffer, and that copy belongs to the platform. Zeroing
//! it is the host runtime's job, on its side of the boundary.

use core::fmt;

use zeroize::Zeroizing;

use pump_shared::Secret;

/// Secret bytes. Swift and Kotlin see the platform byte-array type.
#[derive(Clone, PartialEq, Eq)]
pub struct SecretBytes(Zeroizing<Vec<u8>>);

impl SecretBytes {
    pub fn new(bytes: Vec<u8>) -> Self {
        Self(Zeroizing::new(bytes))
    }

    pub fn expose(&self) -> &[u8] {
        &self.0
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Debug for SecretBytes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SecretBytes({} bytes)", self.0.len())
    }
}

impl From<Secret> for SecretBytes {
    fn from(secret: Secret) -> Self {
        Self::new(secret.expose().to_vec())
    }
}

impl From<SecretBytes> for Secret {
    fn from(bytes: SecretBytes) -> Self {
        Secret::new(bytes.expose().to_vec())
    }
}

uniffi::custom_type!(SecretBytes, Vec<u8>, {
    lower: |secret| secret.expose().to_vec(),
    try_lift: |bytes| Ok(SecretBytes::new(bytes)),
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_never_shows_bytes() {
        let secret = SecretBytes::new(vec![0xAB; 32]);
        assert_eq!(format!("{secret:?}"), "SecretBytes(32 bytes)");
        assert_eq!(secret.len(), 32);
        assert!(!secret.is_empty());
        assert!(SecretBytes::new(Vec::new()).is_empty());
    }

    #[test]
    fn round_trips_through_the_shared_type() {
        let secret = Secret::new(vec![1, 2, 3]);
        let crossed = SecretBytes::from(secret.clone());
        assert_eq!(crossed.expose(), &[1, 2, 3]);
        assert_eq!(Secret::from(crossed.clone()), secret);
        assert_eq!(crossed, SecretBytes::new(vec![1, 2, 3]));
    }
}
