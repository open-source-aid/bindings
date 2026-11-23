//! Every value that crosses the ABI, crossed and crossed back.
//!
//! These are not protocol tests: they check that the thin layer is faithful,
//! which is the only thing this crate is allowed to do.

use osaid_bindings::error::BindingError;
use osaid_bindings::host::action::{Action, Class, Completion, Slot};
use osaid_bindings::host::ble::{BleOp, BleResult, Notification, NotifyKind};
use osaid_bindings::host::driver::{Advertisement, Config, EngineState, Kind};
use osaid_bindings::host::secret::SecretBytes;
use osaid_bindings::pump::command::{
    command_delivers_insulin, command_is_stop_path, command_kind, Command, CommandKind,
};
use osaid_bindings::pump::event::Event;
use osaid_bindings::pump::history::{
    history_sync_id, Alarm, AlarmKind, HistoryEntry, HistoryEvent,
};
use osaid_bindings::pump::outcome::{Failure, Outcome, Refusal, Report};
use osaid_bindings::pump::status::{
    Basal, Bolus, Extended, Nominal, NominalCapabilities, NominalLimits, PumpMode, PumpStatus,
    TherapyState,
};
use osaid_bindings::pump::units::*;

use core::time::Duration;

use pump_shared as ps;

fn uuid(byte: u8) -> ps::Uuid {
    ps::Uuid::from_bytes([byte; 16])
}

fn address() -> ps::Address {
    ps::Address::new([1, 2, 3, 4, 5, 6])
}

#[test]
fn ble_ops_round_trip() {
    let ops = [
        ps::BleOp::Connect { address: address() },
        ps::BleOp::Disconnect,
        ps::BleOp::Read {
            service: uuid(1),
            characteristic: uuid(2),
        },
        ps::BleOp::Write {
            service: uuid(1),
            characteristic: uuid(2),
            data: vec![7, 8],
        },
        ps::BleOp::Subscribe {
            service: uuid(1),
            characteristic: uuid(2),
            kind: ps::NotifyKind::Notification,
        },
        ps::BleOp::Subscribe {
            service: uuid(1),
            characteristic: uuid(2),
            kind: ps::NotifyKind::Indication,
        },
        ps::BleOp::Unsubscribe {
            service: uuid(1),
            characteristic: uuid(2),
        },
        ps::BleOp::RequestFastInterval,
    ];
    for op in ops {
        let crossed = BleOp::from(op.clone());
        assert_eq!(ps::BleOp::from(crossed.clone()), op);
        assert_eq!(crossed, BleOp::from(op));
    }
}

#[test]
fn ble_results_round_trip_and_unknown_ones_cannot_go_back() {
    let results = [
        ps::BleResult::Done,
        ps::BleResult::Data(vec![1]),
        ps::BleResult::Status(9),
        ps::BleResult::Timeout,
        ps::BleResult::Disconnected,
        ps::BleResult::Failed,
    ];
    for result in results {
        let crossed = BleResult::from(result.clone());
        assert_eq!(ps::BleResult::try_from(crossed).unwrap(), result);
    }
    let unknown = BleResult::Unrecognized {
        detail: "Future".to_owned(),
    };
    assert!(matches!(
        ps::BleResult::try_from(unknown),
        Err(BindingError::Invalid { .. })
    ));
}

#[test]
fn notifications_round_trip() {
    let notification = ps::Notification {
        service: uuid(3),
        characteristic: uuid(4),
        data: vec![5, 6],
    };
    let crossed = Notification::from(notification.clone());
    assert_eq!(crossed.data, vec![5, 6]);
    assert_eq!(ps::Notification::from(crossed), notification);
    assert_eq!(
        NotifyKind::from(ps::NotifyKind::Indication),
        NotifyKind::Indication
    );
    assert_eq!(
        ps::NotifyKind::from(NotifyKind::Notification),
        ps::NotifyKind::Notification
    );
}

#[test]
fn slots_and_classes_round_trip() {
    let slots = [
        ps::Slot::Session,
        ps::Slot::PendingSession,
        ps::Slot::Counters,
        ps::Slot::NonceState,
        ps::Slot::Driver(3),
    ];
    for slot in slots {
        let crossed = Slot::from(slot);
        assert_eq!(ps::Slot::try_from(crossed).unwrap(), slot);
    }
    for class in [ps::Class::Secret, ps::Class::Monotonic, ps::Class::Plain] {
        let crossed = Class::from(class);
        assert_eq!(ps::Class::try_from(crossed).unwrap(), class);
    }
    assert!(ps::Slot::try_from(Slot::Unrecognized {
        detail: "x".to_owned()
    })
    .is_err());
    assert!(ps::Class::try_from(Class::Unrecognized {
        detail: "x".to_owned()
    })
    .is_err());
}

#[test]
fn every_action_round_trips_and_keeps_its_id() {
    let id = ps::OpId::new(42);
    let actions = [
        ps::Action::Ble {
            id,
            op: ps::BleOp::Disconnect,
        },
        ps::Action::Store {
            id,
            slot: ps::Slot::Counters,
            class: ps::Class::Monotonic,
            bytes: vec![1, 2],
        },
        ps::Action::Load {
            id,
            slot: ps::Slot::Session,
        },
        ps::Action::Erase {
            id,
            slot: ps::Slot::Session,
        },
        ps::Action::Wrap {
            id,
            plaintext: ps::Secret::new(vec![9; 32]),
        },
        ps::Action::Unwrap {
            id,
            wrapped: vec![3],
        },
        ps::Action::KeyAgree {
            id,
            key: ps::KeyRef::new("server"),
            peer_public: [7; 32],
        },
        ps::Action::Random { id, len: 16 },
        ps::Action::PublicKey {
            id,
            key: ps::KeyRef::new("phone"),
        },
    ];
    for action in actions {
        let crossed = Action::from(action);
        assert_eq!(crossed.id(), 42);
        let back = ps::Action::try_from(crossed).unwrap();
        assert_eq!(back.id(), id);
    }
}

#[test]
fn a_host_public_key_crosses_as_plain_bytes() {
    // It is not a Secret: the engine sends it straight to the device, so it
    // needs neither zeroising nor a redacted Debug.
    let completion = ps::Completion::PublicKey([9; 32]);
    let crossed = Completion::from(completion);
    let Completion::PublicKey { ref bytes } = crossed else {
        panic!("expected a public key, got {crossed:?}");
    };
    assert_eq!(bytes.len(), 32);
    assert!(format!("{crossed:?}").contains('9'), "{crossed:?}");
    assert!(matches!(
        ps::Completion::try_from(crossed),
        Ok(ps::Completion::PublicKey(_))
    ));
}

#[test]
fn a_public_key_of_the_wrong_width_is_rejected() {
    let short = Completion::PublicKey { bytes: vec![0; 8] };
    assert!(matches!(
        ps::Completion::try_from(short),
        Err(BindingError::Invalid { .. })
    ));
}

#[test]
fn malformed_values_are_rejected_not_panicked_on() {
    let short_key = Action::KeyAgree {
        id: 1,
        key: "k".to_owned(),
        peer_public: vec![0; 4],
    };
    assert!(matches!(
        ps::Action::try_from(short_key),
        Err(BindingError::Invalid { .. })
    ));
    let bad_slot = Action::Store {
        id: 1,
        slot: Slot::Unrecognized {
            detail: "x".to_owned(),
        },
        class: Class::Plain,
        bytes: Vec::new(),
    };
    assert!(ps::Action::try_from(bad_slot).is_err());
    let bad_class = Action::Store {
        id: 1,
        slot: Slot::Session,
        class: Class::Unrecognized {
            detail: "x".to_owned(),
        },
        bytes: Vec::new(),
    };
    assert!(ps::Action::try_from(bad_class).is_err());
    assert!(ps::Action::try_from(Action::Load {
        id: 1,
        slot: Slot::Unrecognized {
            detail: "x".to_owned()
        }
    })
    .is_err());
    assert!(ps::Action::try_from(Action::Erase {
        id: 1,
        slot: Slot::Unrecognized {
            detail: "x".to_owned()
        }
    })
    .is_err());
}

#[test]
fn completions_round_trip() {
    let completions = [
        ps::Completion::Ble(ps::BleResult::Done),
        ps::Completion::Stored,
        ps::Completion::Loaded(None),
        ps::Completion::Loaded(Some(vec![1])),
        ps::Completion::Wrapped(vec![2]),
        ps::Completion::Secret(ps::Secret::new(vec![3; 32])),
        ps::Completion::Failed,
    ];
    for completion in completions {
        let crossed = Completion::from(completion);
        let back = ps::Completion::try_from(crossed.clone()).unwrap();
        assert_eq!(Completion::from(back), crossed);
    }
    assert!(ps::Completion::try_from(Completion::Ble {
        result: BleResult::Unrecognized {
            detail: "x".to_owned()
        }
    })
    .is_err());
}

#[test]
fn a_secret_crosses_as_bytes_and_never_prints() {
    let secret = SecretBytes::from(ps::Secret::new(vec![0xAB; 32]));
    assert_eq!(format!("{secret:?}"), "SecretBytes(32 bytes)");
    assert_eq!(ps::Secret::from(secret).expose(), [0xAB; 32]);
    let action = Action::from(ps::Action::Wrap {
        id: ps::OpId::new(1),
        plaintext: ps::Secret::new(vec![1, 2, 3]),
    });
    assert!(!format!("{action:?}").contains("1, 2, 3"));
}

#[test]
fn commands_cross_and_report_what_they_are() {
    let bolus = Command::Bolus { milliunits: 1_000 };
    assert_eq!(command_kind(bolus.clone()).unwrap(), CommandKind::Bolus);
    assert!(command_delivers_insulin(bolus.clone()).unwrap());
    assert!(!command_is_stop_path(bolus).unwrap());
    assert!(command_is_stop_path(Command::Suspend).unwrap());
    assert!(command_is_stop_path(Command::CancelTempBasal).unwrap());
    assert!(command_is_stop_path(Command::CancelExtendedBolus).unwrap());

    let commands = [
        Command::Status,
        Command::Pair {
            address: address(),
            server_key: Some("server".to_owned()),
        },
        Command::Pair {
            address: address(),
            server_key: None,
        },
        Command::Unpair,
        Command::RateBolus {
            milliunits: 100,
            seconds: 600,
        },
        Command::TempBasal {
            rate: TempBasalRate::Percent { percent: 120 },
            seconds: 1_800,
        },
        Command::TempBasal {
            rate: TempBasalRate::Absolute {
                milliunits_per_hour: 500,
            },
            seconds: 1_800,
        },
        Command::ExtendedBolus {
            milliunits: 100,
            seconds: 3_600,
        },
        Command::ReadClock,
        Command::SyncClock,
        Command::ReadHistory { after: 12 },
        Command::ReadBasalProfile,
        Command::SetBasalProfile {
            profile: vec![BasalSegment {
                start_secs: 0,
                milliunits_per_hour: 800,
            }],
        },
        Command::Resume,
        Command::SetDeliveryLimits {
            max_bolus_milliunits: 10_000,
            max_basal_milliunits_per_hour: 3_000,
        },
    ];
    for command in commands {
        assert!(command_kind(command).is_ok());
    }
    assert_eq!(
        command_kind(Command::CancelTempBasal).unwrap(),
        CommandKind::CancelTempBasal
    );
}

#[test]
fn an_invalid_basal_profile_is_an_error_not_a_panic() {
    let empty = Command::SetBasalProfile {
        profile: Vec::new(),
    };
    assert!(matches!(
        command_kind(empty),
        Err(BindingError::Invalid { .. })
    ));
    let late = Command::SetBasalProfile {
        profile: vec![BasalSegment {
            start_secs: 3_600,
            milliunits_per_hour: 100,
        }],
    };
    assert!(command_kind(late).is_err());
}

#[test]
fn command_kinds_cross() {
    let kinds = [
        (ps::CommandKind::Status, CommandKind::Status),
        (ps::CommandKind::Pair, CommandKind::Pair),
        (ps::CommandKind::Unpair, CommandKind::Unpair),
        (ps::CommandKind::Bolus, CommandKind::Bolus),
        (ps::CommandKind::RateBolus, CommandKind::RateBolus),
        (ps::CommandKind::TempBasal, CommandKind::TempBasal),
        (
            ps::CommandKind::CancelTempBasal,
            CommandKind::CancelTempBasal,
        ),
        (ps::CommandKind::ExtendedBolus, CommandKind::ExtendedBolus),
        (
            ps::CommandKind::CancelExtendedBolus,
            CommandKind::CancelExtendedBolus,
        ),
        (ps::CommandKind::ReadClock, CommandKind::ReadClock),
        (ps::CommandKind::SyncClock, CommandKind::SyncClock),
        (ps::CommandKind::ReadHistory, CommandKind::ReadHistory),
        (
            ps::CommandKind::ReadBasalProfile,
            CommandKind::ReadBasalProfile,
        ),
        (
            ps::CommandKind::SetBasalProfile,
            CommandKind::SetBasalProfile,
        ),
        (ps::CommandKind::Suspend, CommandKind::Suspend),
        (ps::CommandKind::Resume, CommandKind::Resume),
        (
            ps::CommandKind::SetDeliveryLimits,
            CommandKind::SetDeliveryLimits,
        ),
        (ps::CommandKind::Stop, CommandKind::Stop),
    ];
    for (shared, crossed) in kinds {
        assert_eq!(CommandKind::from(shared), crossed);
    }
}

#[test]
fn bounds_round_trip_and_round_the_same_way_as_rust() {
    let bolus = BoundsInsulin {
        min: 100,
        max: 25_000,
        tiers: vec![TierInsulin { from: 0, step: 100 }],
    };
    assert!(bounds_insulin_allows(&bolus, 1_000));
    assert!(!bounds_insulin_allows(&bolus, 1_050));
    assert_eq!(bounds_insulin_floor(&bolus, 1_050), Some(1_000));
    assert_eq!(bounds_insulin_nearest(&bolus, 1_060), Some(1_100));
    assert_eq!(bounds_insulin_values(&bolus).len(), 250);

    let basal = BoundsBasalRate {
        min: 20,
        max: 100,
        tiers: vec![TierBasalRate { from: 0, step: 10 }],
    };
    assert!(bounds_basal_rate_allows(&basal, 20));
    assert_eq!(bounds_basal_rate_floor(&basal, 25), Some(20));
    assert_eq!(bounds_basal_rate_nearest(&basal, 29), Some(30));
    assert_eq!(bounds_basal_rate_values(&basal).len(), 9);

    let percent = BoundsPercent {
        min: 0,
        max: 200,
        tiers: vec![TierPercent { from: 0, step: 10 }],
    };
    assert!(bounds_percent_allows(&percent, 100));
    assert_eq!(bounds_percent_floor(&percent, 115), Some(110));
    assert_eq!(bounds_percent_nearest(&percent, 116), Some(120));
    assert_eq!(bounds_percent_values(&percent).len(), 21);

    let duration = BoundsDuration {
        min: 900,
        max: 7_200,
        tiers: vec![TierDuration { from: 0, step: 900 }],
    };
    assert!(bounds_duration_allows(&duration, 1_800));
    assert_eq!(bounds_duration_floor(&duration, 1_000), Some(900));
    assert_eq!(bounds_duration_nearest(&duration, 1_700), Some(1_800));
    assert_eq!(bounds_duration_values(&duration).len(), 8);
}

#[test]
fn outcomes_and_reports_cross() {
    let status = ps::PumpStatus {
        therapy: ps::TherapyState {
            mode: ps::PumpMode::Running,
            basal: ps::Basal::Temporary {
                rate: ps::TempBasalRate::Percent(ps::Percent::new(120)),
                duration: core::time::Duration::from_secs(1_800),
            },
            bolus: ps::Bolus::Immediate {
                requested: ps::InsulinAmount::from_milliunits(1_000),
                delivered: ps::InsulinAmount::from_milliunits(500),
            },
            extended: ps::Extended::Running {
                amount: ps::InsulinAmount::from_milliunits(200),
                duration: core::time::Duration::from_secs(600),
            },
        },
        battery: Some(ps::Percent::new(80)),
        reservoir: Some(ps::InsulinAmount::from_milliunits(12_000)),
        basal_rate: Some(ps::BasalRate::from_milliunits_per_hour(800)),
        clock: Some(ps::PumpTime::from_local_seconds(1_000)),
        delivery_uncertain: true,
        alarms: vec![ps::Alarm {
            kind: ps::AlarmKind::Occlusion,
            code: 7,
        }],
    };
    let crossed = PumpStatus::from(status);
    assert!(crossed.delivery_uncertain);
    assert_eq!(crossed.battery_percent, Some(80));
    assert_eq!(crossed.alarms[0].kind, AlarmKind::Occlusion);
    assert_eq!(crossed.therapy.mode, PumpMode::Running);
    assert!(matches!(crossed.therapy.basal, Basal::Temporary { .. }));
    assert!(matches!(crossed.therapy.bolus, Bolus::Immediate { .. }));
    assert!(matches!(crossed.therapy.extended, Extended::Running { .. }));

    let outcomes = [
        ps::Outcome::Completed(ps::Report::Done),
        ps::Outcome::Completed(ps::Report::Clock(ps::PumpTime::from_local_seconds(5))),
        ps::Outcome::Completed(ps::Report::Delivered(ps::InsulinAmount::from_milliunits(
            100,
        ))),
        ps::Outcome::Completed(ps::Report::History(Vec::new())),
        ps::Outcome::Approximated(ps::Approximation::Bolus {
            requested: ps::InsulinAmount::from_milliunits(2_000),
            delivered: ps::InsulinAmount::from_milliunits(1_850),
            duration: core::time::Duration::from_secs(60),
        }),
        ps::Outcome::Refused(ps::Refusal::NotBuilt),
        ps::Outcome::Unknown,
        ps::Outcome::Failed(ps::Failure::Status(0x8B)),
    ];
    for outcome in outcomes {
        let _ = Outcome::from(outcome);
    }
    assert_eq!(Refusal::from(ps::Refusal::Busy), Refusal::Busy);
    assert_eq!(
        Failure::from(ps::Failure::KeyRejected),
        Failure::KeyRejected
    );
    assert!(matches!(
        Report::from(ps::Report::Status(ps::PumpStatus::default())),
        Report::Status { .. }
    ));
}

#[test]
fn history_crosses_with_its_raw_bytes() {
    let entry = ps::HistoryEntry {
        sequence: ps::Sequence::new(12),
        time: ps::UtcTime::from_unix_millis(1_700_000_000_000),
        pump_time: Some(ps::PumpTime::from_local_seconds(50)),
        event: ps::HistoryEvent::Bolus {
            kind: ps::BolusKind::Smb,
            requested: ps::InsulinAmount::from_milliunits(200),
            delivered: ps::InsulinAmount::from_milliunits(200),
            duration: core::time::Duration::from_secs(30),
            automatic: true,
            in_progress: false,
        },
        raw: vec![0xDE, 0xAD],
    };
    let crossed = HistoryEntry::from(entry);
    assert_eq!(crossed.sequence, 12);
    assert_eq!(crossed.raw, vec![0xDE, 0xAD]);
    assert_eq!(crossed.pump_local_seconds, Some(50));
    assert!(matches!(crossed.event, HistoryEvent::Bolus { .. }));
    assert_eq!(history_sync_id("ABC".to_owned(), 12), "ABC-12");

    let events = [
        ps::HistoryEvent::ExtendedBolusEnded {
            delivered: ps::InsulinAmount::from_milliunits(100),
        },
        ps::HistoryEvent::TempBasal {
            rate: ps::TempBasalRate::Absolute(ps::BasalRate::from_milliunits_per_hour(500)),
            duration: core::time::Duration::from_secs(1_800),
            phase: ps::TempBasalPhase::Started,
        },
        ps::HistoryEvent::Suspended,
        ps::HistoryEvent::Resumed,
        ps::HistoryEvent::BasalProfileChanged,
        ps::HistoryEvent::Rewound,
        ps::HistoryEvent::Replaced(ps::Component::Cannula),
        ps::HistoryEvent::ClockChanged {
            previous: ps::PumpTime::from_local_seconds(1),
        },
        ps::HistoryEvent::Alarm(ps::Alarm {
            kind: ps::AlarmKind::BatteryLow,
            code: 1,
        }),
        ps::HistoryEvent::AlarmCleared(ps::Alarm {
            kind: ps::AlarmKind::NoDelivery,
            code: 2,
        }),
        ps::HistoryEvent::DailyTotal {
            day: ps::PumpTime::from_local_seconds(0),
            amount: ps::InsulinAmount::from_milliunits(30_000),
        },
        ps::HistoryEvent::LimitChanged(ps::LimitChange::BolusStep {
            value: ps::InsulinAmount::from_milliunits(10),
        }),
    ];
    for event in events {
        let _ = HistoryEvent::from(event);
    }
    assert_eq!(
        Alarm::from(ps::Alarm {
            kind: ps::AlarmKind::AutoOff,
            code: 3
        })
        .code,
        3
    );
}

#[test]
fn events_cross() {
    let events = [
        ps::Event::Started {
            id: ps::CommandId::new(1),
        },
        ps::Event::Progress {
            id: ps::CommandId::new(1),
            delivered: ps::InsulinAmount::from_milliunits(100),
            requested: ps::InsulinAmount::from_milliunits(200),
        },
        ps::Event::Finished {
            id: ps::CommandId::new(1),
            outcome: ps::Outcome::Unknown,
        },
        ps::Event::StateChanged(ps::EngineState::Paired),
        ps::Event::TherapyChanged(ps::TherapyState::default()),
        ps::Event::Log {
            level: 3,
            line: "hello".to_owned(),
        },
    ];
    for event in events {
        let _ = Event::from(event);
    }
    for state in [
        ps::EngineState::Loading,
        ps::EngineState::Unpaired,
        ps::EngineState::Pairing,
        ps::EngineState::Paired,
        ps::EngineState::KeyRejected,
    ] {
        let crossed = EngineState::from(state);
        assert_eq!(ps::EngineState::try_from(crossed).unwrap(), state);
    }
    assert!(ps::EngineState::try_from(EngineState::Unrecognized).is_err());
    assert_eq!(Kind::from(ps::Kind::Pump), Kind::Pump);
    assert_eq!(Kind::from(ps::Kind::Cgm), Kind::Cgm);
    assert_eq!(
        TherapyState::from(ps::TherapyState::default()).mode,
        PumpMode::Unknown
    );
}

#[test]
fn an_advertisement_borrows_as_the_shared_type() {
    let advertisement = Advertisement {
        address: address(),
        name: Some("YpsoPump_1234".to_owned()),
        service_uuids: vec![uuid(1)],
        manufacturer_id: Some(0x1234),
        manufacturer_data: vec![1, 2],
        rssi: -60,
    };
    advertisement.with(|shared| {
        assert_eq!(shared.name, Some("YpsoPump_1234"));
        assert_eq!(shared.rssi, -60);
        assert_eq!(shared.manufacturer, Some((0x1234, [1, 2].as_slice())));
    });
    let anonymous = Advertisement {
        address: address(),
        name: None,
        service_uuids: Vec::new(),
        manufacturer_id: None,
        manufacturer_data: Vec::new(),
        rssi: 0,
    };
    anonymous.with(|shared| assert!(shared.manufacturer.is_none()));
}

#[test]
fn config_crosses() {
    let config = Config {
        address: Some(address()),
        utc_offset_secs: 3_600,
    };
    let shared = ps::Config::from(config);
    assert_eq!(shared.utc_offset_secs, 3_600);
    assert_eq!(shared.address, Some(address()));
    assert_eq!(ps::Config::from(Config::default()), ps::Config::default());
}

#[test]
fn shared_errors_keep_their_meaning() {
    assert_eq!(
        BindingError::from(ps::SubmitError::QueueFull),
        BindingError::QueueFull
    );
    assert_eq!(
        BindingError::from(ps::CompletionError::NotPending),
        BindingError::NotPending
    );
    assert_eq!(
        BindingError::from(ps::CompletionError::Mismatch),
        BindingError::Mismatch
    );
    assert!(!BindingError::QueueFull.to_string().is_empty());
    assert!(BindingError::NotImplemented {
        what: "x".to_owned()
    }
    .to_string()
    .contains('x'));
    assert!(BindingError::Unsupported {
        detail: "y".to_owned()
    }
    .to_string()
    .contains('y'));
}

#[test]
fn the_fixed_width_values_are_length_checked_on_the_way_in() {
    assert_eq!(
        osaid_bindings::error::address(&[1, 2, 3, 4, 5, 6]).unwrap(),
        address()
    );
    assert!(matches!(
        osaid_bindings::error::address(&[1, 2]),
        Err(BindingError::Invalid { .. })
    ));
    assert_eq!(osaid_bindings::error::uuid(&[7; 16]).unwrap(), uuid(7));
    assert!(osaid_bindings::error::uuid(&[7; 15]).is_err());
    assert_eq!(
        osaid_bindings::error::public_key(&[9; 32]).unwrap(),
        [9; 32]
    );
    assert!(osaid_bindings::error::public_key(&[9; 31]).is_err());
}

#[test]
fn device_info_reaches_the_host_as_an_event() {
    let event = Event::from(ps::Event::DeviceInfo(Box::new(ps::DeviceInfo {
        serial: "10155555".to_owned(),
        firmware: "1.7.4".to_owned(),
        variant: "ypso",
        limits: ps::Limits {
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
            temp_basal_percent: None,
            temp_basal_absolute: None,
            temp_basal_duration: ps::Bounds::uniform(
                core::time::Duration::from_secs(900),
                core::time::Duration::from_secs(7_200),
                core::time::Duration::from_secs(900),
            ),
            extended_bolus_duration: None,
        },
        capabilities: ps::Capabilities {
            reservoir_capacity: None,
            max_profile_segments: 24,
            min_segment_duration: core::time::Duration::from_secs(3_600),
            history: true,
            daily_totals: false,
            patch: false,
        },
    })));
    match event {
        Event::DeviceInfo { info } => assert_eq!(info.serial, "10155555"),
        other => panic!("expected device info, got {other:?}"),
    }
}

const INSULIN_TIERS: &[ps::Tier<ps::InsulinAmount>] = &[ps::Tier {
    from: ps::InsulinAmount::ZERO,
    step: ps::InsulinAmount::from_milliunits(50),
}];
const RATE_TIERS: &[ps::Tier<ps::BasalRate>] = &[ps::Tier {
    from: ps::BasalRate::from_milliunits_per_hour(0),
    step: ps::BasalRate::from_milliunits_per_hour(10),
}];
const PERCENT_TIERS: &[ps::Tier<ps::Percent>] = &[ps::Tier {
    from: ps::Percent::new(0),
    step: ps::Percent::new(10),
}];
const QUARTER_TIERS: &[ps::Tier<Duration>] = &[ps::Tier {
    from: Duration::ZERO,
    step: Duration::from_secs(900),
}];

#[test]
fn nominal_bounds_cross_as_their_own_records_and_only_enumerate() {
    let family = ps::Nominal {
        limits: ps::NominalLimits {
            bolus: Some(ps::NominalBounds {
                min: ps::InsulinAmount::from_milliunits(100),
                max: ps::InsulinAmount::from_milliunits(500),
                tiers: INSULIN_TIERS,
            }),
            basal: Some(ps::NominalBounds {
                min: ps::BasalRate::from_milliunits_per_hour(20),
                max: ps::BasalRate::from_milliunits_per_hour(100),
                tiers: RATE_TIERS,
            }),
            temp_basal_percent: Some(ps::NominalBounds {
                min: ps::Percent::new(0),
                max: ps::Percent::new(200),
                tiers: PERCENT_TIERS,
            }),
            temp_basal_absolute: None,
            temp_basal_duration: Some(ps::NominalBounds {
                min: Duration::from_secs(900),
                max: Duration::from_secs(3600),
                tiers: QUARTER_TIERS,
            }),
            extended_bolus_duration: None,
        },
        capabilities: ps::NominalCapabilities {
            reservoir_capacity: Some(ps::InsulinAmount::from_milliunits(160_000)),
            max_profile_segments: Some(24),
            min_segment_duration: Some(Duration::from_secs(3600)),
            history: true,
            daily_totals: false,
            patch: false,
        },
    };

    let crossed = Nominal::from(&family);
    let bolus = crossed.limits.bolus.clone().expect("a bolus grid");
    assert_eq!(bolus.min, 100);
    assert_eq!(bolus.max, 500);
    assert_eq!(
        nominal_insulin_values(&bolus),
        [100, 150, 200, 250, 300, 350, 400, 450, 500]
    );

    let basal = crossed.limits.basal.clone().expect("a basal grid");
    assert_eq!(nominal_basal_rate_values(&basal).len(), 9);
    let percent = crossed
        .limits
        .temp_basal_percent
        .clone()
        .expect("a percent grid");
    assert_eq!(nominal_percent_values(&percent).len(), 21);
    let duration = crossed
        .limits
        .temp_basal_duration
        .clone()
        .expect("a duration grid");
    assert_eq!(nominal_duration_values(&duration), [900, 1800, 2700, 3600]);
    assert_eq!(crossed.limits.temp_basal_absolute, None);
    assert_eq!(crossed.limits.extended_bolus_duration, None);

    assert_eq!(
        crossed.capabilities.reservoir_capacity_milliunits,
        Some(160_000)
    );
    assert_eq!(crossed.capabilities.max_profile_segments, Some(24));
    assert_eq!(crossed.capabilities.min_segment_seconds, Some(3600));
    assert!(crossed.capabilities.history);
    assert!(!crossed.capabilities.daily_totals);
    assert!(!crossed.capabilities.patch);
    assert!(format!("{crossed:?}").contains("Nominal"));
}

#[test]
fn a_family_that_evidences_nothing_crosses_as_nothing() {
    let crossed = Nominal::from(&ps::Nominal::default());
    assert_eq!(
        crossed,
        Nominal {
            limits: NominalLimits {
                bolus: None,
                basal: None,
                temp_basal_percent: None,
                temp_basal_absolute: None,
                temp_basal_duration: None,
                extended_bolus_duration: None,
            },
            capabilities: NominalCapabilities {
                reservoir_capacity_milliunits: None,
                max_profile_segments: None,
                min_segment_seconds: None,
                history: false,
                daily_totals: false,
                patch: false,
            },
        }
    );
}
