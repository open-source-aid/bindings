//! Device simulators, in process (`sim` feature).
//!
//! This is the real simulator from the driver crate, the same one the Rust
//! e2e tests run against, not a second implementation written for the host.
//! A Swift or Kotlin test harness can therefore drive a complete pairing,
//! status and history session with no hardware and no BLE stack: hand each
//! `BleOp` from `pollAction` to [`SimDeviceHandle::handle`] and hand the
//! `BleResult` straight back to `handleCompletion`.
//!
//! `sim` is off by default and MUST be off in app builds (guideline 03): it
//! links simulator code and fixed test keys that protect nothing.

use std::sync::Mutex;

use pump_shared::{KeyRef, SimDevice, Timestamp};

use crate::error::BindingError;
use crate::host::ble::{BleOp, BleResult, Notification};

/// Same rule and same reasoning as `EngineHandle`: a simulator is plain
/// state, so it may move between threads, but one harness thread at a time.
struct Serial(Box<dyn SimDevice>);

// SAFETY: see `crate::engine::Serial`. A `SimDevice` is plain state with no
// thread affinity; the `Mutex` below is what keeps two threads out of it.
unsafe impl Send for Serial {}

impl core::fmt::Debug for SimDeviceHandle {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("SimDeviceHandle")
    }
}

/// One simulated device.
#[derive(uniffi::Object)]
pub struct SimDeviceHandle {
    device: Mutex<Serial>,
}

impl SimDeviceHandle {
    fn new(device: Box<dyn SimDevice>) -> Self {
        Self {
            device: Mutex::new(Serial(device)),
        }
    }

    fn with<T>(&self, f: impl FnOnce(&mut dyn SimDevice) -> T) -> T {
        let mut guard = self
            .device
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        f(guard.0.as_mut())
    }
}

#[uniffi::export]
impl SimDeviceHandle {
    /// Perform one BLE operation against the simulated device.
    pub fn handle(&self, op: BleOp, now_millis: u64) -> BleResult {
        let op = op.into();
        self.with(|device| device.handle(&op, Timestamp::from_millis(now_millis)))
            .into()
    }

    /// X25519 with the simulator's own key, for drivers that pair with one.
    pub fn key_agree(
        &self,
        key: String,
        peer_public: Vec<u8>,
    ) -> Result<Option<Vec<u8>>, BindingError> {
        let peer_public = crate::error::public_key(&peer_public)?;
        let key = KeyRef::new(key);
        Ok(self
            .with(|device| device.key_agree(&key, &peer_public))
            .map(|shared| shared.to_vec()))
    }

    /// Anything the simulated device would push on its own.
    pub fn poll_notification(&self, now_millis: u64) -> Option<Notification> {
        self.with(|device| device.poll_notification(Timestamp::from_millis(now_millis)))
            .map(Into::into)
    }
}

/// A simulated YpsoPump with a fixed pairing key and password.
///
/// `key` is 32 bytes, `password` 16. Both are test values: they are not
/// secrets and the simulator logs nothing that matters.
#[cfg(feature = "driver-ypso")]
#[uniffi::export]
pub fn ypso_simulator(
    key: Vec<u8>,
    password: Vec<u8>,
) -> Result<std::sync::Arc<SimDeviceHandle>, BindingError> {
    let key = <[u8; pump_ypso::protocol::KEY_LEN]>::try_from(key.as_slice())
        .map_err(|_| BindingError::invalid("a YpsoPump pairing key is 32 bytes"))?;
    let password = <[u8; pump_ypso::sim::PASSWORD_LEN]>::try_from(password.as_slice())
        .map_err(|_| BindingError::invalid("a YpsoPump password is 16 bytes"))?;
    Ok(std::sync::Arc::new(SimDeviceHandle::new(Box::new(
        pump_ypso::sim::YpsoSim::new(key, password),
    ))))
}
