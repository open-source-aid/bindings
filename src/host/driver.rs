//! The static description of a driver, as records.
//!
//! `DriverInfo` is a compile-time constant on the Rust side
//! (`&'static DriverInfo`). A host cannot hold a Rust reference, so it asks a
//! [`DriverHandle`](crate::registry::DriverHandle) for this record, which is
//! a copy. Everything a driver will ever ask the host for is in here, so a
//! host can prepare permissions, keychain entries and GATT discovery up front
//! (guideline 04).

use pump_shared::{
    Address, DriverInfo as PsDriverInfo, EngineState as PsEngineState, Feature as PsFeature,
    GattChar as PsGattChar, Kind as PsKind, Uuid, Variant as PsVariant,
};
use pump_shared::{Advertisement as PsAdvertisement, BleMatch as PsBleMatch, Config as PsConfig};

use crate::error::BindingError;
use crate::host::ble::NotifyKind;
use crate::pump::command::CommandKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum Kind {
    Pump,
    Cgm,
    Unrecognized,
}

impl From<PsKind> for Kind {
    fn from(kind: PsKind) -> Self {
        match kind {
            PsKind::Pump => Self::Pump,
            PsKind::Cgm => Self::Cgm,
            _ => Self::Unrecognized,
        }
    }
}

/// How a scan result is filtered down to this driver.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct BleMatch {
    pub name_prefixes: Vec<String>,
    pub service_uuids: Vec<Uuid>,
    pub manufacturer_id: Option<u16>,
}

impl From<PsBleMatch> for BleMatch {
    fn from(ble: PsBleMatch) -> Self {
        Self {
            name_prefixes: ble.name_prefixes.iter().map(|p| (*p).to_owned()).collect(),
            service_uuids: ble.service_uuids.to_vec(),
            manufacturer_id: ble.manufacturer_id,
        }
    }
}

/// One characteristic the engine may touch. A host MUST refuse a subscription
/// to a characteristic that is not declared here with a `notify` kind (E9).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct GattChar {
    pub name: String,
    pub service: Uuid,
    pub characteristic: Uuid,
    pub notify: Option<NotifyKind>,
}

impl From<&PsGattChar> for GattChar {
    fn from(gatt: &PsGattChar) -> Self {
        Self {
            name: gatt.name.to_owned(),
            service: gatt.service,
            characteristic: gatt.characteristic,
            notify: gatt.notify.map(Into::into),
        }
    }
}

/// A model inside the device family this driver covers.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct Variant {
    pub id: String,
    pub name: String,
}

impl From<&PsVariant> for Variant {
    fn from(variant: &PsVariant) -> Self {
        Self {
            id: variant.id.to_owned(),
            name: variant.name.to_owned(),
        }
    }
}

/// A Cargo feature, and whether this build has it compiled in.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct Feature {
    pub name: String,
    pub enabled: bool,
}

impl From<&PsFeature> for Feature {
    fn from(feature: &PsFeature) -> Self {
        Self {
            name: feature.name.to_owned(),
            enabled: feature.enabled,
        }
    }
}

/// A storage slot the engine uses, and how it must be stored.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct SlotDeclaration {
    pub slot: crate::host::action::Slot,
    pub class: crate::host::action::Class,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct DriverInfo {
    pub id: String,
    pub name: String,
    pub kind: Kind,
    pub version: String,
    /// MUST equal [`api_version()`](crate::api_version) or the host refuses
    /// the driver.
    pub api_version: u16,
    pub protocol: String,
    pub ble: BleMatch,
    pub gatt: Vec<GattChar>,
    pub variants: Vec<Variant>,
    /// The pump commands compiled into this build. A CGM driver leaves this
    /// empty and lists its own commands elsewhere (Q5).
    pub commands: Vec<CommandKind>,
    pub features: Vec<Feature>,
    pub slots: Vec<SlotDeclaration>,
    /// `api_version` matches this build of the shared contract.
    pub compatible: bool,
}

impl From<&'static PsDriverInfo> for DriverInfo {
    fn from(info: &'static PsDriverInfo) -> Self {
        Self {
            id: info.id.to_owned(),
            name: info.name.to_owned(),
            kind: info.kind.into(),
            version: info.version.to_owned(),
            api_version: info.api_version,
            protocol: info.protocol.to_owned(),
            ble: info.ble.into(),
            gatt: info.gatt.iter().map(Into::into).collect(),
            variants: info.variants.iter().map(Into::into).collect(),
            commands: info.commands.iter().copied().map(Into::into).collect(),
            features: info.features.iter().map(Into::into).collect(),
            slots: info
                .slots
                .iter()
                .map(|(slot, class)| SlotDeclaration {
                    slot: (*slot).into(),
                    class: (*class).into(),
                })
                .collect(),
            compatible: info.is_compatible(),
        }
    }
}

/// One scan result, as the platform saw it.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct Advertisement {
    pub address: Address,
    pub name: Option<String>,
    pub service_uuids: Vec<Uuid>,
    pub manufacturer_id: Option<u16>,
    pub manufacturer_data: Vec<u8>,
    pub rssi: i8,
}

impl Advertisement {
    /// Borrow this record as the shared type for the length of `f`.
    pub fn with<T>(&self, f: impl FnOnce(&PsAdvertisement<'_>) -> T) -> T {
        let manufacturer = self
            .manufacturer_id
            .map(|id| (id, self.manufacturer_data.as_slice()));
        let advertisement = PsAdvertisement {
            address: self.address,
            name: self.name.as_deref(),
            service_uuids: &self.service_uuids,
            manufacturer,
            rssi: self.rssi,
        };
        f(&advertisement)
    }
}

/// What the app knows about this device before the engine runs.
#[derive(Debug, Clone, PartialEq, Eq, Default, uniffi::Record)]
pub struct Config {
    pub address: Option<Address>,
    /// Seconds east of UTC, for pumps that keep a local clock.
    pub utc_offset_secs: i32,
}

impl From<Config> for PsConfig {
    fn from(config: Config) -> Self {
        Self {
            address: config.address,
            utc_offset_secs: config.utc_offset_secs,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum EngineState {
    Loading,
    Unpaired,
    Pairing,
    Paired,
    KeyRejected,
    Unrecognized,
}

impl From<PsEngineState> for EngineState {
    fn from(state: PsEngineState) -> Self {
        match state {
            PsEngineState::Loading => Self::Loading,
            PsEngineState::Unpaired => Self::Unpaired,
            PsEngineState::Pairing => Self::Pairing,
            PsEngineState::Paired => Self::Paired,
            PsEngineState::KeyRejected => Self::KeyRejected,
            _ => Self::Unrecognized,
        }
    }
}

impl TryFrom<EngineState> for PsEngineState {
    type Error = BindingError;

    fn try_from(state: EngineState) -> Result<Self, Self::Error> {
        Ok(match state {
            EngineState::Loading => Self::Loading,
            EngineState::Unpaired => Self::Unpaired,
            EngineState::Pairing => Self::Pairing,
            EngineState::Paired => Self::Paired,
            EngineState::KeyRejected => Self::KeyRejected,
            EngineState::Unrecognized => return Err(BindingError::invalid("unknown engine state")),
        })
    }
}
