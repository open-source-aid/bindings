//! What a host sees before it talks to any device.

use osaid_bindings::error::BindingError;
use osaid_bindings::host::driver::{driver_nominal_for, Advertisement, Config, Kind};
use osaid_bindings::registry::{
    api_version, cgm_drivers, driver_by_id, drivers, match_advertisement, new_cgm_engine,
};

fn ypso_advertisement(name: &str) -> Advertisement {
    Advertisement {
        address: pump_shared::Address::new([0xEC, 0x2A, 0xF0, 1, 2, 3]),
        name: Some(name.to_owned()),
        service_uuids: Vec::new(),
        manufacturer_id: None,
        manufacturer_data: Vec::new(),
        rssi: -55,
    }
}

#[test]
fn the_registry_lists_what_this_build_linked() {
    let drivers = drivers();
    assert_eq!(drivers.len(), cfg!(feature = "driver-ypso") as usize);
    for driver in &drivers {
        let info = driver.info();
        assert_eq!(info.id, driver.id());
        assert!(
            info.compatible,
            "{} is built against another contract",
            info.id
        );
        assert_eq!(info.api_version, api_version());
        assert!(!info.variants.is_empty());
        assert!(!format!("{driver:?}").is_empty());
    }
}

#[cfg(feature = "driver-ypso")]
#[test]
fn the_ypso_driver_declares_its_whole_footprint() {
    let driver = driver_by_id("ypso".to_owned()).expect("ypso is linked");
    let info = driver.info();
    assert_eq!(info.kind, Kind::Pump);
    assert_eq!(info.protocol, "ypso-ble");
    assert!(
        !info.gatt.is_empty(),
        "a driver declares every characteristic"
    );
    assert!(
        !info.slots.is_empty(),
        "a driver declares every storage slot"
    );
    assert_eq!(info.ble.name_prefixes, vec!["YpsoPump_".to_owned()]);

    // `delivery` is what this build was compiled with, no more and no less.
    let delivery = info
        .features
        .iter()
        .find(|feature| feature.name == "delivery")
        .expect("the driver reports the delivery feature");
    assert_eq!(delivery.enabled, cfg!(feature = "delivery"));
    assert_eq!(
        info.commands
            .contains(&osaid_bindings::pump::command::CommandKind::Bolus),
        cfg!(feature = "delivery")
    );
    // A stop path is never behind a feature (guideline 08).
    assert!(info
        .commands
        .contains(&osaid_bindings::pump::command::CommandKind::CancelTempBasal));
}

#[test]
fn an_unknown_driver_id_is_not_a_driver() {
    assert!(driver_by_id("nope".to_owned()).is_none());
}

#[cfg(feature = "driver-ypso")]
#[test]
fn advertisements_are_matched_to_a_driver_and_a_model() {
    let matches = match_advertisement(ypso_advertisement("YpsoPump_12345"));
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].driver_id, "ypso");
    assert_eq!(matches[0].variant.id, "ypso");

    assert!(match_advertisement(ypso_advertisement("Dexcom G7")).is_empty());

    let driver = driver_by_id("ypso".to_owned()).unwrap();
    assert!(driver.matches(ypso_advertisement("YpsoPump_1")).is_some());
    assert!(driver.matches(ypso_advertisement("Other")).is_none());
}

#[cfg(feature = "driver-ypso")]
#[test]
fn an_engine_is_made_for_one_known_model() {
    let driver = driver_by_id("ypso".to_owned()).unwrap();
    let engine = driver
        .new_engine("ypso".to_owned(), Config::default())
        .expect("the ypso variant exists");
    assert!(!format!("{engine:?}").is_empty());

    let wrong = driver.new_engine("ypso-made-up".to_owned(), Config::default());
    assert!(matches!(wrong, Err(BindingError::Invalid { .. })));
}

#[test]
fn cgm_support_is_an_honest_stub() {
    assert!(cgm_drivers().is_empty());
    let engine = new_cgm_engine("dexcom".to_owned(), "g7".to_owned(), Config::default());
    assert!(matches!(engine, Err(BindingError::NotImplemented { .. })));
}

#[cfg(feature = "driver-ypso")]
#[test]
fn a_setup_screen_can_be_drawn_before_any_pump_is_connected() {
    use osaid_bindings::pump::units::{nominal_duration_values, nominal_insulin_values};

    let driver = driver_by_id("ypso".to_owned()).expect("ypso is linked");
    let info = driver.info();

    // This is the whole point: no device, no connect, and still a grid.
    let bolus = info
        .nominal
        .limits
        .bolus
        .clone()
        .expect("the YpsoPump family declares its bolus grid");
    let volumes = nominal_insulin_values(&bolus);
    assert_eq!(volumes.first(), Some(&100));
    assert_eq!(volumes.last(), Some(&25_000));
    assert_eq!(
        info.nominal.capabilities.reservoir_capacity_milliunits,
        Some(160_000)
    );
    assert_eq!(info.nominal.capabilities.max_profile_segments, Some(24));

    // The pump has no absolute temp basal, and says so rather than guessing.
    assert_eq!(info.nominal.limits.temp_basal_absolute, None);
    let extended = info
        .nominal
        .limits
        .extended_bolus_duration
        .clone()
        .expect("the family has a native extended bolus");
    assert_eq!(nominal_duration_values(&extended).last(), Some(&43_200));

    // Every model of this family shares the declaration.
    for variant in &info.variants {
        assert_eq!(variant.nominal, None);
        assert_eq!(driver_nominal_for(&info, variant), info.nominal);
    }
}
