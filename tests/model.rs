//! The data model, unit by unit. Limits and capabilities are what an app's
//! UI is built from, so every field is checked, not sampled.

use core::time::Duration;

use osaid_bindings::pump::history::{
    AlarmKind, BolusKind, Component, HistoryEvent, LimitChange, TempBasalPhase,
};
use osaid_bindings::pump::outcome::{Approximation, Failure, Outcome, Refusal, Report};
use osaid_bindings::pump::status::{
    limits_allow_extended_bolus, limits_allow_temp_basal, Basal, Bolus, DeviceInfo, Extended,
    Limits, PumpMode,
};
use osaid_bindings::pump::units::*;

use pump_shared as ps;

fn shared_limits() -> ps::Limits {
    ps::Limits {
        bolus: ps::Bounds::uniform(
            ps::InsulinAmount::from_milliunits(100),
            ps::InsulinAmount::from_milliunits(25_000),
            ps::InsulinAmount::from_milliunits(100),
        ),
        basal: ps::Bounds::uniform(
            ps::BasalRate::from_milliunits_per_hour(20),
            ps::BasalRate::from_milliunits_per_hour(2_500),
            ps::BasalRate::from_milliunits_per_hour(10),
        ),
        temp_basal_percent: Some(ps::Bounds::uniform(
            ps::Percent::new(0),
            ps::Percent::new(200),
            ps::Percent::new(10),
        )),
        temp_basal_absolute: Some(ps::Bounds::uniform(
            ps::BasalRate::from_milliunits_per_hour(0),
            ps::BasalRate::from_milliunits_per_hour(2_500),
            ps::BasalRate::from_milliunits_per_hour(10),
        )),
        temp_basal_duration: ps::Bounds::uniform(
            Duration::from_secs(900),
            Duration::from_secs(7_200),
            Duration::from_secs(900),
        ),
        extended_bolus_duration: Some(ps::Bounds::uniform(
            Duration::from_secs(900),
            Duration::from_secs(7_200),
            Duration::from_secs(900),
        )),
    }
}

fn shared_capabilities() -> ps::Capabilities {
    ps::Capabilities {
        reservoir_capacity: Some(ps::InsulinAmount::from_milliunits(160_000)),
        max_profile_segments: 24,
        min_segment_duration: Duration::from_secs(3_600),
        history: true,
        daily_totals: true,
        patch: false,
    }
}

#[test]
fn device_info_carries_the_whole_limits_table() {
    let info = ps::DeviceInfo {
        serial: "10155555".to_owned(),
        firmware: "1.7.4".to_owned(),
        variant: "ypso",
        limits: shared_limits(),
        capabilities: shared_capabilities(),
    };
    let crossed = DeviceInfo::from(info);
    assert_eq!(crossed.serial, "10155555");
    assert_eq!(crossed.firmware, "1.7.4");
    assert_eq!(crossed.variant, "ypso");
    assert_eq!(
        crossed.capabilities.reservoir_capacity_milliunits,
        Some(160_000)
    );
    assert_eq!(crossed.capabilities.max_profile_segments, 24);
    assert_eq!(crossed.capabilities.min_segment_seconds, 3_600);
    assert!(crossed.capabilities.history);
    assert!(crossed.capabilities.daily_totals);
    assert!(!crossed.capabilities.patch);

    let limits = &crossed.limits;
    assert_eq!(limits.bolus.min, 100);
    assert_eq!(limits.bolus.max, 25_000);
    assert_eq!(limits.basal.tiers[0].step, 10);
    assert_eq!(limits.temp_basal_percent.as_ref().unwrap().max, 200);
    assert_eq!(limits.temp_basal_absolute.as_ref().unwrap().max, 2_500);
    assert_eq!(limits.temp_basal_duration.min, 900);
    assert_eq!(limits.extended_bolus_duration.as_ref().unwrap().max, 7_200);
}

#[test]
fn limits_answer_what_the_device_accepts() {
    let limits = Limits::from_shared(&shared_limits());
    assert!(limits_allow_temp_basal(
        &limits,
        TempBasalRate::Percent { percent: 120 },
        1_800
    ));
    assert!(!limits_allow_temp_basal(
        &limits,
        TempBasalRate::Percent { percent: 125 },
        1_800
    ));
    assert!(limits_allow_temp_basal(
        &limits,
        TempBasalRate::Absolute {
            milliunits_per_hour: 500
        },
        1_800
    ));
    assert!(limits_allow_extended_bolus(&limits, 1_000, 1_800));
    assert!(!limits_allow_extended_bolus(&limits, 1_050, 1_800));

    let bare = Limits {
        bolus: limits.bolus.clone(),
        basal: limits.basal.clone(),
        temp_basal_percent: None,
        temp_basal_absolute: None,
        temp_basal_duration: limits.temp_basal_duration.clone(),
        extended_bolus_duration: None,
    };
    assert!(!limits_allow_temp_basal(
        &bare,
        TempBasalRate::Percent { percent: 100 },
        1_800
    ));
    assert!(!limits_allow_extended_bolus(&bare, 1_000, 1_800));
}

#[test]
fn basal_profiles_cross_both_ways() {
    let segments = vec![
        BasalSegment {
            start_secs: 0,
            milliunits_per_hour: 800,
        },
        BasalSegment {
            start_secs: 28_800,
            milliunits_per_hour: 1_200,
        },
    ];
    let report = Report::from(ps::Report::Profile(
        ps::BasalProfile::new(segments.iter().copied().map(Into::into).collect()).unwrap(),
    ));
    match report {
        Report::Profile { segments: back } => assert_eq!(back, segments),
        other => panic!("expected a profile, got {other:?}"),
    }
}

#[test]
fn therapy_states_cross_in_every_shape() {
    let states = [
        (ps::PumpMode::Unknown, PumpMode::Unknown),
        (ps::PumpMode::Off, PumpMode::Off),
        (ps::PumpMode::Suspended, PumpMode::Suspended),
        (ps::PumpMode::Running, PumpMode::Running),
    ];
    for (shared, crossed) in states {
        assert_eq!(PumpMode::from(shared), crossed);
    }
    assert_eq!(Basal::from(ps::Basal::Unknown), Basal::Unknown);
    assert_eq!(Basal::from(ps::Basal::Scheduled), Basal::Scheduled);
    assert_eq!(Bolus::from(ps::Bolus::Unknown), Bolus::Unknown);
    assert_eq!(Bolus::from(ps::Bolus::Idle), Bolus::Idle);
    assert_eq!(Extended::from(ps::Extended::Unknown), Extended::Unknown);
    assert_eq!(Extended::from(ps::Extended::Idle), Extended::Idle);
}

#[test]
fn every_refusal_and_failure_crosses() {
    let refusals = [
        (ps::Refusal::NotBuilt, Refusal::NotBuilt),
        (ps::Refusal::NotPaired, Refusal::NotPaired),
        (ps::Refusal::NotRunning, Refusal::NotRunning),
        (ps::Refusal::NotSuspended, Refusal::NotSuspended),
        (ps::Refusal::OutOfLimits, Refusal::OutOfLimits),
        (ps::Refusal::Busy, Refusal::Busy),
        (ps::Refusal::Unsupported, Refusal::Unsupported),
    ];
    for (shared, crossed) in refusals {
        assert_eq!(Refusal::from(shared), crossed);
    }
    let failures = [
        (ps::Failure::KeyRejected, Failure::KeyRejected),
        (ps::Failure::Host, Failure::Host),
        (ps::Failure::Disconnected, Failure::Disconnected),
        (ps::Failure::Protocol, Failure::Protocol),
        (ps::Failure::Status(0x8B), Failure::Status { code: 0x8B }),
    ];
    for (shared, crossed) in failures {
        assert_eq!(Failure::from(shared), crossed);
    }
    assert!(matches!(
        Approximation::from(ps::Approximation::Bolus {
            requested: ps::InsulinAmount::from_milliunits(2_000),
            delivered: ps::InsulinAmount::from_milliunits(1_850),
            duration: Duration::from_secs(60),
        }),
        Approximation::Bolus {
            delivered_milliunits: 1_850,
            ..
        }
    ));
    assert!(matches!(
        Outcome::from(ps::Outcome::Refused(ps::Refusal::OutOfLimits)),
        Outcome::Refused {
            refusal: Refusal::OutOfLimits
        }
    ));
}

#[test]
fn every_history_enum_crosses() {
    let bolus_kinds = [
        (ps::BolusKind::Normal, BolusKind::Normal),
        (ps::BolusKind::Smb, BolusKind::Smb),
        (ps::BolusKind::Priming, BolusKind::Priming),
        (ps::BolusKind::Extended, BolusKind::Extended),
        (ps::BolusKind::Combined, BolusKind::Combined),
    ];
    for (shared, crossed) in bolus_kinds {
        assert_eq!(BolusKind::from(shared), crossed);
    }
    for (shared, crossed) in [
        (ps::TempBasalPhase::Started, TempBasalPhase::Started),
        (ps::TempBasalPhase::Running, TempBasalPhase::Running),
        (ps::TempBasalPhase::Ended, TempBasalPhase::Ended),
    ] {
        assert_eq!(TempBasalPhase::from(shared), crossed);
    }
    for (shared, crossed) in [
        (ps::Component::Reservoir, Component::Reservoir),
        (ps::Component::Cannula, Component::Cannula),
        (ps::Component::Tubing, Component::Tubing),
        (ps::Component::Battery, Component::Battery),
        (ps::Component::Pump, Component::Pump),
    ] {
        assert_eq!(Component::from(shared), crossed);
    }
    for (shared, crossed) in [
        (ps::AlarmKind::Occlusion, AlarmKind::Occlusion),
        (ps::AlarmKind::ReservoirLow, AlarmKind::ReservoirLow),
        (ps::AlarmKind::ReservoirEmpty, AlarmKind::ReservoirEmpty),
        (ps::AlarmKind::BatteryLow, AlarmKind::BatteryLow),
        (ps::AlarmKind::BatteryEmpty, AlarmKind::BatteryEmpty),
        (ps::AlarmKind::AutoOff, AlarmKind::AutoOff),
        (ps::AlarmKind::NoDelivery, AlarmKind::NoDelivery),
        (ps::AlarmKind::Other, AlarmKind::Other),
    ] {
        assert_eq!(AlarmKind::from(shared), crossed);
    }
    let changes = [
        ps::LimitChange::MaxBolus {
            value: ps::InsulinAmount::from_milliunits(10_000),
            previous: ps::InsulinAmount::from_milliunits(8_000),
        },
        ps::LimitChange::MaxBasal {
            value: ps::BasalRate::from_milliunits_per_hour(3_000),
            previous: ps::BasalRate::from_milliunits_per_hour(2_000),
        },
        ps::LimitChange::BolusStep {
            value: ps::InsulinAmount::from_milliunits(10),
        },
    ];
    for change in changes {
        assert!(!matches!(
            LimitChange::from(change),
            LimitChange::Unrecognized
        ));
    }
    assert!(matches!(
        HistoryEvent::from(ps::HistoryEvent::Bolus {
            kind: ps::BolusKind::Normal,
            requested: ps::InsulinAmount::from_milliunits(100),
            delivered: ps::InsulinAmount::from_milliunits(100),
            duration: Duration::from_secs(10),
            automatic: false,
            in_progress: true,
        }),
        HistoryEvent::Bolus {
            in_progress: true,
            ..
        }
    ));
}

#[test]
fn tiered_bounds_keep_every_tier() {
    let tiered = ps::Bounds {
        min: ps::BasalRate::from_milliunits_per_hour(20),
        max: ps::BasalRate::from_milliunits_per_hour(2_500),
        tiers: vec![
            ps::Tier {
                from: ps::BasalRate::from_milliunits_per_hour(0),
                step: ps::BasalRate::from_milliunits_per_hour(10),
            },
            ps::Tier {
                from: ps::BasalRate::from_milliunits_per_hour(1_000),
                step: ps::BasalRate::from_milliunits_per_hour(20),
            },
            ps::Tier {
                from: ps::BasalRate::from_milliunits_per_hour(2_000),
                step: ps::BasalRate::from_milliunits_per_hour(100),
            },
        ],
    };
    let crossed = BoundsBasalRate::from_shared(&tiered);
    assert_eq!(crossed.tiers.len(), 3);
    assert_eq!(crossed.to_shared(), tiered);
    // The step changes with the tier, and the bindings do not reimplement it.
    assert_eq!(bounds_basal_rate_floor(&crossed, 1_030), Some(1_020));
    assert_eq!(bounds_basal_rate_floor(&crossed, 2_150), Some(2_100));
    assert_eq!(bounds_basal_rate_nearest(&crossed, 2_190), Some(2_200));
}

#[test]
fn temp_basal_rates_cross_both_ways() {
    for rate in [
        ps::TempBasalRate::Percent(ps::Percent::new(130)),
        ps::TempBasalRate::Absolute(ps::BasalRate::from_milliunits_per_hour(750)),
    ] {
        let crossed = TempBasalRate::from(rate);
        assert_eq!(ps::TempBasalRate::from(crossed), rate);
    }
}
