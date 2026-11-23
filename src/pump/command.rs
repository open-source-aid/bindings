//! `Command` and `CommandKind`.
//!
//! A command is a device operation, never a therapy decision. Which ones this
//! build accepts is in `DriverInfo.commands`; asking for one that is not
//! there finishes as `Outcome::Refused(Refusal::NotBuilt)` and sends nothing
//! to the device.

use core::time::Duration;

use pump_shared::{Address, BasalRate, InsulinAmount, KeyRef, Sequence};
use pump_shared::{Command as PsCommand, CommandKind as PsCommandKind};

use crate::error::BindingError;
use crate::pump::units::{BasalSegment, TempBasalRate};

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum CommandKind {
    Status,
    Pair,
    Unpair,
    Bolus,
    RateBolus,
    TempBasal,
    CancelTempBasal,
    ExtendedBolus,
    CancelExtendedBolus,
    ReadClock,
    SyncClock,
    ReadHistory,
    ReadBasalProfile,
    SetBasalProfile,
    Suspend,
    Resume,
    SetDeliveryLimits,
    /// `Engine::stop()`, which is always accepted (E4).
    Stop,
    Unrecognized,
}

impl From<PsCommandKind> for CommandKind {
    fn from(kind: PsCommandKind) -> Self {
        match kind {
            PsCommandKind::Status => Self::Status,
            PsCommandKind::Pair => Self::Pair,
            PsCommandKind::Unpair => Self::Unpair,
            PsCommandKind::Bolus => Self::Bolus,
            PsCommandKind::RateBolus => Self::RateBolus,
            PsCommandKind::TempBasal => Self::TempBasal,
            PsCommandKind::CancelTempBasal => Self::CancelTempBasal,
            PsCommandKind::ExtendedBolus => Self::ExtendedBolus,
            PsCommandKind::CancelExtendedBolus => Self::CancelExtendedBolus,
            PsCommandKind::ReadClock => Self::ReadClock,
            PsCommandKind::SyncClock => Self::SyncClock,
            PsCommandKind::ReadHistory => Self::ReadHistory,
            PsCommandKind::ReadBasalProfile => Self::ReadBasalProfile,
            PsCommandKind::SetBasalProfile => Self::SetBasalProfile,
            PsCommandKind::Suspend => Self::Suspend,
            PsCommandKind::Resume => Self::Resume,
            PsCommandKind::SetDeliveryLimits => Self::SetDeliveryLimits,
            PsCommandKind::Stop => Self::Stop,
            _ => Self::Unrecognized,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum Command {
    Status,
    Pair {
        address: Address,
        /// Name of a host-held key for the key agreement, when the protocol
        /// has one.
        server_key: Option<String>,
    },
    Unpair,
    Bolus {
        milliunits: u32,
    },
    RateBolus {
        milliunits: u32,
        seconds: u64,
    },
    TempBasal {
        rate: TempBasalRate,
        seconds: u64,
    },
    CancelTempBasal,
    ExtendedBolus {
        milliunits: u32,
        seconds: u64,
    },
    CancelExtendedBolus,
    ReadClock,
    SyncClock,
    ReadHistory {
        after: u32,
    },
    ReadBasalProfile,
    SetBasalProfile {
        profile: Vec<BasalSegment>,
    },
    Suspend,
    Resume,
    SetDeliveryLimits {
        max_bolus_milliunits: u32,
        max_basal_milliunits_per_hour: u32,
    },
}

impl TryFrom<Command> for PsCommand {
    type Error = BindingError;

    fn try_from(command: Command) -> Result<Self, Self::Error> {
        Ok(match command {
            Command::Status => Self::Status,
            Command::Pair {
                address,
                server_key,
            } => Self::Pair {
                address,
                server_key: server_key.map(KeyRef::new),
            },
            Command::Unpair => Self::Unpair,
            Command::Bolus { milliunits } => Self::Bolus {
                amount: InsulinAmount::from_milliunits(milliunits),
            },
            Command::RateBolus {
                milliunits,
                seconds,
            } => Self::RateBolus {
                amount: InsulinAmount::from_milliunits(milliunits),
                duration: Duration::from_secs(seconds),
            },
            Command::TempBasal { rate, seconds } => Self::TempBasal {
                rate: rate.into(),
                duration: Duration::from_secs(seconds),
            },
            Command::CancelTempBasal => Self::CancelTempBasal,
            Command::ExtendedBolus {
                milliunits,
                seconds,
            } => Self::ExtendedBolus {
                amount: InsulinAmount::from_milliunits(milliunits),
                duration: Duration::from_secs(seconds),
            },
            Command::CancelExtendedBolus => Self::CancelExtendedBolus,
            Command::ReadClock => Self::ReadClock,
            Command::SyncClock => Self::SyncClock,
            Command::ReadHistory { after } => Self::ReadHistory {
                after: Sequence::new(after),
            },
            Command::ReadBasalProfile => Self::ReadBasalProfile,
            Command::SetBasalProfile { profile } => Self::SetBasalProfile {
                profile: crate::pump::units::profile(profile)?,
            },
            Command::Suspend => Self::Suspend,
            Command::Resume => Self::Resume,
            Command::SetDeliveryLimits {
                max_bolus_milliunits,
                max_basal_milliunits_per_hour,
            } => Self::SetDeliveryLimits {
                max_bolus: InsulinAmount::from_milliunits(max_bolus_milliunits),
                max_basal: BasalRate::from_milliunits_per_hour(max_basal_milliunits_per_hour),
            },
        })
    }
}

/// What kind of command this is, without submitting it.
#[uniffi::export]
pub fn command_kind(command: Command) -> Result<CommandKind, BindingError> {
    Ok(PsCommand::try_from(command)?.kind().into())
}

/// `true` when this command is a stop path. A stop path is never gated behind
/// the `delivery` feature (guideline 08).
#[uniffi::export]
pub fn command_is_stop_path(command: Command) -> Result<bool, BindingError> {
    Ok(PsCommand::try_from(command)?.is_stop_path())
}

/// `true` when carrying this command out delivers insulin.
#[uniffi::export]
pub fn command_delivers_insulin(command: Command) -> Result<bool, BindingError> {
    Ok(PsCommand::try_from(command)?.delivers_insulin())
}
