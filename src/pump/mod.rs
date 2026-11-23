//! The pump half of the contract: commands, outcomes, therapy state, limits
//! and history.
//!
//! Everything here is pump-specific. A CGM driver uses [`crate::host`] and a
//! CGM model of its own (Q5); nothing in this module is in its way.

pub mod command;
pub mod event;
pub mod history;
pub mod outcome;
pub mod status;
pub mod units;
