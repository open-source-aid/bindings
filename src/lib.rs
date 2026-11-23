//! UniFFI bindings for the open-source-aid sans-IO device drivers.
//!
//! This crate is the *thin* FFI-friendly layer guideline 13 describes, and
//! nothing more. It turns `&'static DriverInfo` and `Box<dyn Engine>` into
//! handle objects, `Action`/`Completion` into records, and the generic
//! `Bounds<T>` into one record per unit. It adds **no** logic: no protocol,
//! no retry policy, no reconnect policy, no unit conversion beyond the
//! integer repacking the ABI forces. Anything Swift and Kotlin would both
//! need belongs in Rust, in a driver or in a shared crate.
//!
//! # What a host does with it
//!
//! 1. [`drivers()`](registry::drivers) says what was linked into this build.
//! 2. [`match_advertisement()`](registry::match_advertisement) turns a scan
//!    result into a driver and a model.
//! 3. [`DriverHandle::new_engine`](registry::DriverHandle::new_engine) makes
//!    one engine for one device.
//! 4. The host runs the loop from guideline 04 against that engine, **on one
//!    serial queue per engine**. See [`engine`] for the rule and why the lock
//!    inside is not permission to ignore it.
//!
//! # Layout
//!
//! * [`host`] — the generic half: `Action`, `Completion`, `BleOp`, `Slot`,
//!   `Class`, `Secret`, `DriverInfo`. If Q5 splits `pump-shared` into
//!   `device-shared` + `pump-shared` + `cgm-shared`, this module is what
//!   follows `device-shared`, and it is kept free of pump types so that split
//!   stays cheap.
//! * [`pump`] — commands, outcomes, therapy state, limits, history.
//! * [`registry`] — which drivers this build links.
//! * [`session`] — validating a stored session a host was handed from
//!   elsewhere, for drivers that cannot pair a device themselves.
//! * [`engine`] — the engine handle.
//! * `sim` — device simulators in process, behind the `sim` feature.

#![warn(missing_debug_implementations)]

pub mod engine;
pub mod error;
pub mod host;
pub mod pump;
pub mod registry;
pub mod session;
#[cfg(feature = "sim")]
pub mod sim;

pub use engine::EngineHandle;
pub use error::BindingError;
pub use registry::{api_version, driver_by_id, drivers, match_advertisement, DriverHandle};
pub use session::{ypso_inspect_session, SessionBlobError, SessionSummary};

uniffi::setup_scaffolding!("osaid");
