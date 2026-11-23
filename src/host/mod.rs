//! The generic half of the contract: everything a CGM driver, an uploader or
//! a pump driver needs equally.
//!
//! Q5 proposes moving these types out of `pump-shared` into a
//! `device-shared` crate. This module is the FFI mirror of that split: it
//! depends on nothing in [`crate::pump`] except `CommandKind`, which
//! `DriverInfo` itself carries upstream. When the split happens, this module
//! moves with it and the pump module keeps compiling on top.

pub mod action;
pub mod ble;
pub mod driver;
pub mod secret;
