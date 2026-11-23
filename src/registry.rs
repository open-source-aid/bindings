//! The driver registry.
//!
//! This crate is one library that statically links every driver the app
//! ships. Which drivers that is, is a build-time choice: one Cargo feature
//! per driver (`driver-ypso`, …), plus the cross-cutting ones (`delivery`,
//! `sim`). `drivers()` lists exactly what was linked, so a host never
//! discovers a driver it did not compile in, and `DriverInfo.features` says
//! which features that driver was built with.
//!
//! Adding a driver is three lines: a Cargo feature, an entry in
//! [`DRIVERS`], and nothing else. A CGM driver registers the same way; see
//! the README for the one extra piece it needs (a `CgmEngineHandle`
//! alongside [`EngineHandle`], because `CgmEngine` is a separate trait until
//! Q5 is settled).

use std::sync::Arc;

use pump_shared::Driver;

use crate::engine::EngineHandle;
use crate::error::BindingError;
use crate::host::driver::{Advertisement, Config, DriverInfo, Variant};

/// Every driver linked into this build, in registration order.
static DRIVERS: &[&(dyn Driver + Sync)] = &[
    #[cfg(feature = "driver-ypso")]
    &pump_ypso::DRIVER,
];

impl core::fmt::Debug for DriverHandle {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "DriverHandle({})", self.driver.info().id)
    }
}

/// A linked driver. Holds `&'static dyn Driver`, which a host cannot, so
/// everything a host needs from it is a method here.
#[derive(uniffi::Object)]
pub struct DriverHandle {
    driver: &'static (dyn Driver + Sync),
}

impl DriverHandle {
    fn new(driver: &'static (dyn Driver + Sync)) -> Arc<Self> {
        Arc::new(Self { driver })
    }

    fn variant(&self, id: &str) -> Result<&'static pump_shared::Variant, BindingError> {
        self.driver
            .info()
            .variants
            .iter()
            .find(|variant| variant.id == id)
            .ok_or_else(|| BindingError::invalid(format!("no variant {id:?} in this driver")))
    }
}

#[uniffi::export]
impl DriverHandle {
    /// Everything this driver will ever ask the host for.
    pub fn info(&self) -> DriverInfo {
        self.driver.info().into()
    }

    pub fn id(&self) -> String {
        self.driver.info().id.to_owned()
    }

    /// The model this advertisement is, or `nil` when it is not this driver's
    /// device.
    pub fn matches(&self, advertisement: Advertisement) -> Option<Variant> {
        advertisement
            .with(|advertisement| self.driver.matches(advertisement))
            .map(Into::into)
    }

    /// A fresh engine for one device. The caller owns it and MUST drive it
    /// from one serial executor; see [`EngineHandle`].
    pub fn new_engine(
        &self,
        variant_id: String,
        config: Config,
    ) -> Result<Arc<EngineHandle>, BindingError> {
        if !self.driver.info().is_compatible() {
            return Err(BindingError::unsupported(format!(
                "driver {} was built against contract version {}, these bindings speak {}",
                self.driver.info().id,
                self.driver.info().api_version,
                pump_shared::API_VERSION
            )));
        }
        let variant = self.variant(&variant_id)?;
        Ok(Arc::new(EngineHandle::new(
            self.driver.new_engine(variant, config.into()),
        )))
    }
}

/// Which driver an advertisement belongs to, and which model it is.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct DriverMatch {
    pub driver_id: String,
    pub variant: Variant,
}

/// Every driver linked into this build.
#[uniffi::export]
pub fn drivers() -> Vec<Arc<DriverHandle>> {
    DRIVERS.iter().copied().map(DriverHandle::new).collect()
}

/// A linked driver by its `DriverInfo.id`.
#[uniffi::export]
pub fn driver_by_id(id: String) -> Option<Arc<DriverHandle>> {
    DRIVERS
        .iter()
        .copied()
        .find(|driver| driver.info().id == id)
        .map(DriverHandle::new)
}

/// Every linked driver that claims this scan result. Normally none or one;
/// the host decides what to do if two drivers claim the same device.
#[uniffi::export]
pub fn match_advertisement(advertisement: Advertisement) -> Vec<DriverMatch> {
    advertisement.with(|advertisement| {
        DRIVERS
            .iter()
            .filter_map(|driver| {
                driver.matches(advertisement).map(|variant| DriverMatch {
                    driver_id: driver.info().id.to_owned(),
                    variant: variant.into(),
                })
            })
            .collect()
    })
}

/// The contract version these bindings were built against. A driver whose
/// `DriverInfo.api_version` differs is refused.
#[uniffi::export]
pub fn api_version() -> u16 {
    pump_shared::API_VERSION
}

/// CGM drivers linked into this build.
///
/// Stub: no CGM driver exists yet, and `cgm-shared`'s `CgmEngine` is a
/// separate trait from `pump-shared`'s `Engine` until Q5 is settled. The
/// entry point exists so an app can call it today and get an empty list
/// rather than a missing symbol tomorrow.
#[uniffi::export]
pub fn cgm_drivers() -> Vec<Arc<DriverHandle>> {
    Vec::new()
}

/// An engine for a CGM.
///
/// Stub: returns `NotImplemented` until a CGM driver is linked. A CGM engine
/// is not a `pump_shared::Engine` — `cgm-shared` defines `CgmEngine`, with
/// the same methods and the same rules E1-E10 but its own command, outcome
/// and event types — so it needs a `CgmEngineHandle` next to `EngineHandle`,
/// not a new variant of it. See the README.
#[uniffi::export]
pub fn new_cgm_engine(
    driver_id: String,
    _variant_id: String,
    _config: Config,
) -> Result<Arc<EngineHandle>, BindingError> {
    Err(BindingError::not_implemented(format!(
        "no CGM driver is linked into this build (asked for {driver_id:?})"
    )))
}
