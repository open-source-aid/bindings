//! The host loop of guideline 04, run in process against the real driver
//! simulator — the same thing a Swift or Kotlin test harness does, with the
//! same records, one serial thread, and no BLE stack.
//!
//! What this proves is not protocol behaviour (the driver's own e2e suite
//! does that): it proves that every value the loop needs survives the shape
//! the bindings give it.

#![cfg(all(feature = "sim", feature = "driver-ypso"))]

use std::collections::BTreeMap;
use std::sync::Arc;

use osaid_bindings::host::action::{Action, Completion};
use osaid_bindings::host::ble::BleResult;
use osaid_bindings::host::driver::{Config, EngineState};
use osaid_bindings::pump::command::Command;
use osaid_bindings::pump::event::Event;
use osaid_bindings::pump::outcome::{Outcome, Refusal};
use osaid_bindings::registry::driver_by_id;
use osaid_bindings::sim::{ypso_simulator, SimDeviceHandle};
use osaid_bindings::EngineHandle;

const KEY: [u8; 32] = [0x42; 32];
const PASSWORD: [u8; 16] = *b"0123456789abcdef";

/// A host runtime, as small as one can be: storage in a map, a counter for
/// "random", an identity keystore, and the simulator for BLE.
struct Host {
    sim: Arc<SimDeviceHandle>,
    storage: BTreeMap<String, Vec<u8>>,
    random: u8,
    now_millis: u64,
    /// Which kinds of action the engine asked for, in order.
    asked: Vec<&'static str>,
}

impl Host {
    fn perform(&mut self, action: Action) -> Completion {
        self.asked.push(match action {
            Action::Ble { .. } => "ble",
            Action::Store { .. } => "store",
            Action::Load { .. } => "load",
            Action::Erase { .. } => "erase",
            Action::Wrap { .. } => "wrap",
            Action::Unwrap { .. } => "unwrap",
            Action::KeyAgree { .. } => "key-agree",
            Action::Random { .. } => "random",
        });
        match action {
            Action::Ble { op, .. } => Completion::Ble {
                result: self.sim.handle(op, self.now_millis),
            },
            Action::Store { slot, bytes, .. } => {
                self.storage.insert(format!("{slot:?}"), bytes);
                Completion::Stored
            }
            Action::Load { slot, .. } => Completion::Loaded {
                bytes: self.storage.get(&format!("{slot:?}")).cloned(),
            },
            Action::Erase { slot, .. } => {
                self.storage.remove(&format!("{slot:?}"));
                Completion::Stored
            }
            // A real host uses the platform keystore here. This one is the
            // identity function, which is enough to exercise the records.
            Action::Wrap { plaintext, .. } => Completion::Wrapped {
                bytes: plaintext.expose().to_vec(),
            },
            Action::Unwrap { wrapped, .. } => Completion::Secret {
                secret: osaid_bindings::host::secret::SecretBytes::new(wrapped),
            },
            Action::KeyAgree {
                key, peer_public, ..
            } => match self.sim.key_agree(key, peer_public) {
                Ok(Some(shared)) => Completion::Secret {
                    secret: osaid_bindings::host::secret::SecretBytes::new(shared),
                },
                _ => Completion::Failed,
            },
            Action::Random { len, .. } => {
                self.random = self.random.wrapping_add(1);
                Completion::Secret {
                    secret: osaid_bindings::host::secret::SecretBytes::new(vec![
                        self.random;
                        len as usize
                    ]),
                }
            }
        }
    }
}

/// Runs the loop until the engine is idle, or `budget` steps have passed.
fn run(engine: &EngineHandle, host: &mut Host, budget: usize) -> Vec<Event> {
    let mut events = Vec::new();
    for _ in 0..budget {
        while let Some(event) = engine.poll_event() {
            events.push(event);
        }
        if let Some(notification) = host.sim.poll_notification(host.now_millis) {
            engine.handle_notification(notification);
            continue;
        }
        let Some(action) = engine.poll_action() else {
            if let Some(timeout) = engine.poll_timeout() {
                host.now_millis = timeout;
                engine.handle_timeout(timeout);
                continue;
            }
            break;
        };
        let id = action.id();
        let completion = host.perform(action);
        engine
            .handle_completion(id, completion)
            .expect("the completion matches the one outstanding action (E1)");
        host.now_millis += 10;
    }
    while let Some(event) = engine.poll_event() {
        events.push(event);
    }
    events
}

fn harness() -> (Arc<EngineHandle>, Host) {
    let engine = driver_by_id("ypso".to_owned())
        .expect("ypso is linked")
        .new_engine(
            "ypso".to_owned(),
            Config {
                address: Some(pump_shared::Address::new([0xEC, 0x2A, 0xF0, 1, 2, 3])),
                utc_offset_secs: 0,
            },
        )
        .expect("the ypso variant exists");
    let host = Host {
        sim: ypso_simulator(KEY.to_vec(), PASSWORD.to_vec()).expect("well-formed test keys"),
        storage: BTreeMap::new(),
        random: 0,
        now_millis: 0,
        asked: Vec::new(),
    };
    (engine, host)
}

#[test]
fn a_command_finishes_exactly_once() {
    let (engine, mut host) = harness();
    let id = engine.submit(Command::Status).expect("the queue is empty");

    let events = run(&engine, &mut host, 5_000);

    let finished: Vec<_> = events
        .iter()
        .filter(|event| matches!(event, Event::Finished { id: finished, .. } if *finished == id))
        .collect();
    assert_eq!(finished.len(), 1, "E5: exactly one Finished per submit");
}

#[test]
fn an_unpaired_engine_loads_its_session_and_refuses() {
    let (engine, mut host) = harness();
    engine.submit(Command::Status).unwrap();
    let events = run(&engine, &mut host, 5_000);

    assert_eq!(
        host.asked,
        vec!["load"],
        "an unpaired engine asks the host for its session and nothing else"
    );
    assert!(events.iter().any(
        |event| matches!(event, Event::StateChanged { state } if *state == EngineState::Unpaired)
    ));
    assert!(
        events.iter().any(|event| matches!(
            event,
            Event::Finished {
                outcome: Outcome::Refused {
                    refusal: Refusal::NotPaired
                },
                ..
            }
        )),
        "Refused means nothing was sent to the device"
    );
}

#[test]
fn stop_is_always_accepted() {
    let (engine, mut host) = harness();
    let id = engine.stop();
    let events = run(&engine, &mut host, 5_000);
    assert!(events
        .iter()
        .any(|event| matches!(event, Event::Finished { id: f, .. } if *f == id)));
}

#[test]
fn a_completion_for_another_operation_is_refused() {
    let (engine, mut host) = harness();
    engine.submit(Command::Status).unwrap();
    // Drive one step so an action is outstanding.
    let action = loop {
        while engine.poll_event().is_some() {}
        if let Some(action) = engine.poll_action() {
            break action;
        }
    };
    let wrong_id = action.id().wrapping_add(1_000);
    assert!(
        engine
            .handle_completion(wrong_id, Completion::Failed)
            .is_err(),
        "E1: a completion for another id changes nothing"
    );
    let completion = host.perform(action.clone());
    assert!(engine.handle_completion(action.id(), completion).is_ok());
}

#[test]
fn the_simulator_answers_ble_operations() {
    let sim = ypso_simulator(KEY.to_vec(), PASSWORD.to_vec()).unwrap();
    let result = sim.handle(
        osaid_bindings::host::ble::BleOp::Connect {
            address: pump_shared::Address::new([0; 6]),
        },
        0,
    );
    assert!(matches!(result, BleResult::Done | BleResult::Failed));
    assert!(!format!("{sim:?}").is_empty());
}

#[test]
fn simulator_keys_are_length_checked() {
    assert!(ypso_simulator(vec![0; 4], PASSWORD.to_vec()).is_err());
    assert!(ypso_simulator(KEY.to_vec(), vec![0; 2]).is_err());
    let sim = ypso_simulator(KEY.to_vec(), PASSWORD.to_vec()).unwrap();
    assert!(sim.key_agree("server".to_owned(), vec![0; 4]).is_err());
    assert!(sim.key_agree("server".to_owned(), vec![0; 32]).is_ok());
}

#[test]
fn every_engine_method_is_reachable_from_a_host() {
    let (engine, _host) = harness();
    assert_eq!(engine.state(), EngineState::Loading);
    assert_eq!(
        engine.therapy().mode,
        osaid_bindings::pump::status::PumpMode::Unknown
    );
    // Asking costs nothing and sends nothing.
    assert!(engine.guard(Command::Status).is_ok());
    // Clock and link inputs are accepted at any time.
    engine.handle_wall_clock(1_700_000_000_000);
    engine.handle_link_lost();
    engine.handle_timeout(engine.poll_timeout().unwrap_or_default());
    engine.handle_notification(osaid_bindings::host::ble::Notification {
        service: pump_shared::Uuid::from_bytes([0; 16]),
        characteristic: pump_shared::Uuid::from_bytes([1; 16]),
        data: vec![0],
    });
    // A command the bindings cannot even build never reaches the engine.
    assert!(engine
        .submit(Command::SetBasalProfile {
            profile: Vec::new()
        })
        .is_err());
    assert!(engine
        .guard(Command::SetBasalProfile {
            profile: Vec::new()
        })
        .is_err());
    assert!(engine
        .handle_completion(
            0,
            Completion::Ble {
                result: BleResult::Unrecognized {
                    detail: "x".to_owned()
                }
            }
        )
        .is_err());
}
