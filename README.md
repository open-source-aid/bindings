# bindings

The FFI layer that exposes the open-source-aid sans-IO device drivers to Swift and Kotlin,
built with [UniFFI](https://github.com/mozilla/uniffi-rs). One Rust library, statically linked
into the app, that carries every driver that app ships.

This is the **Bindings** piece of [guideline 13](https://github.com/open-source-aid/guidelines/blob/main/13-app-integration.md).
The other two pieces are not here: the **host runtime** (CoreBluetooth, keychain, storage,
timers, and the host loop) is written once per platform, and the **app adapter** is written once
per app.

Status: skeleton. It builds, it is tested, its generated Swift compiles, and the whole host loop
runs in process against the real driver simulator. What is not finished is listed under
[Gaps](#gaps).

## The rule: one serial executor per engine

**An engine is not `Sync` and MUST NOT be called concurrently.** One `EngineHandle` is driven by
one serial queue or one thread, for its whole life:

```swift
let queue = DispatchQueue(label: "pump.\(serial)")      // serial, one per engine

queue.async {
    while let event = engine.pollEvent() { adapter.on(event) }
    guard let action = engine.pollAction() else { return }
    let completion = host.perform(action)               // may be async on the platform side
    try engine.handleCompletion(id: actionId(action: action), completion: completion)
}
```

`EngineHandle` holds a mutex, because a UniFFI object is shared as `Arc<T>` and so must be
`Send + Sync`. That mutex makes a concurrent call *safe*; it cannot make it *correct*. A second
queue interleaving `submit` and `pollAction` still breaks rule E1 (at most one action
outstanding at a time), and the engine will report `CompletionError`, not crash. Treat the lock
as a seat belt, not as permission.

Everything returns promptly (E2): nothing here blocks, sleeps, or starts a thread.

## What an app links

An app picks its drivers and its features at build time:

```toml
[dependencies]
bindings = { git = "https://github.com/open-source-aid/bindings", branch = "main",
             default-features = false, features = ["driver-ypso", "delivery"] }
```

| Feature | Default | Meaning |
|---|---|---|
| `driver-ypso` | on | Link the YpsoPump driver. One feature per driver. |
| `delivery` | **off** | Commands that deliver insulin (`Bolus`, `TempBasal`, `ExtendedBolus`, `SetBasalProfile`). Stop paths are never behind it (guideline 08). |
| `sim` | **off** | Device simulators, in process. MUST be off in app builds. |
| `ypso-opn` | off | YpsoPump without CryptoService. A protocol variant. |
| `hardware` | off | Reserved: code that can only run against real hardware, excluded from the coverage gate (Q4). Nothing is behind it today. |

Whatever was compiled in is visible at run time, so a host never has to guess:

```swift
for driver in drivers() {
    let info = driver.info()
    info.features        // [Feature(name: "delivery", enabled: false), …]
    info.commands        // the CommandKinds this build accepts
    info.gatt            // every characteristic the engine may touch
    info.slots           // every storage slot, and its Class
    info.compatible      // info.apiVersion == apiVersion()
}
```

## Using it from Swift

```swift
// 1. Scan, and ask which driver owns this device.
let found = matchAdvertisement(advertisement: Advertisement(
    address: peripheral.address, name: peripheral.name,
    serviceUuids: peripheral.services, manufacturerId: nil,
    manufacturerData: Data(), rssi: rssi))

// 2. Make one engine for one device.
guard let match = found.first,
      let driver = driverById(id: match.driverId) else { return }
let engine = try driver.newEngine(
    variantId: match.variant.id,
    config: Config(address: peripheral.address, utcOffsetSecs: 7200))

// 3. Run the host loop on one serial queue (above), and submit commands on it.
let id = try engine.submit(command: .bolus(milliunits: 2_500))
```

Rounding belongs in Rust, so both platforms round identically:

```swift
let safe = boundsInsulinFloor(bounds: limits.bolus, value: requested)   // Loop and Trio
let near = boundsInsulinNearest(bounds: limits.bolus, value: requested) // AAPS
```

Unit conversion is the adapter's job, once, at the edge: the ABI carries milliunits,
milliunits per hour, whole percent and whole seconds, never a float.

### The simulator in a Swift test harness

With `sim` on, a test harness runs the driver's own simulator in process — the same one the
Rust e2e suite uses, not a second implementation:

```swift
let sim = try ypsoSimulator(key: key, password: password)
// in the host loop, instead of CoreBluetooth:
case .ble(_, let op): return .ble(result: sim.handle(op: op, nowMillis: now))
```

## Layout

```
src/
├── lib.rs          the crate's contract and the UniFFI entry point
├── error.rs        BindingError: the one error that crosses
├── host/           the generic half (Q5: this is what follows `device-shared`)
│   ├── action.rs   Action, Completion, Slot, Class
│   ├── ble.rs      Uuid, Address, BleOp, BleResult, Notification
│   ├── driver.rs   DriverInfo, BleMatch, GattChar, Variant, Advertisement, Config
│   └── secret.rs   Secret, as bytes, still zeroised on the Rust side
├── pump/           the pump half
│   ├── command.rs  Command, CommandKind
│   ├── event.rs    Event
│   ├── history.rs  HistoryEntry, HistoryEvent, Alarm
│   ├── outcome.rs  Outcome, Report, Refusal, Failure, Approximation
│   ├── status.rs   PumpStatus, TherapyState, Limits, Capabilities, DeviceInfo
│   └── units.rs    Bounds monomorphised per unit, and the rounding helpers
├── engine.rs       EngineHandle
├── registry.rs     drivers(), driverById(), matchAdvertisement()
├── session.rs      ypsoInspectSession(): validating a session a host was handed
└── sim.rs          SimDeviceHandle (feature `sim`)
```

### What crossing the ABI changes, and nothing else

| Rust contract | Across the ABI | Why |
|---|---|---|
| `&'static DriverInfo` | `DriverHandle` object + a `DriverInfo` record | A host cannot hold a Rust reference |
| `Box<dyn Engine>` | `EngineHandle` object | Same, plus the serial-executor rule above |
| `Bounds<T>` | `BoundsInsulin`, `BoundsBasalRate`, `BoundsPercent`, `BoundsDuration` | Generics do not cross |
| `Secret` | bytes, with a `Zeroizing` buffer on the Rust side | The ABI carries bytes |
| `InsulinAmount`, `Duration`, … | `u32`, `u64` in the unit the shared crate uses | Newtypes do not cross |
| `OpId`, `CommandId`, `Sequence` | `u32` | Same |
| `Option<&'static Variant>` | `Variant?` record | Same |

The bindings add **no** logic: no protocol, no retry policy, no reconnect policy, no unit
conversion beyond this repacking. The exported helpers (`boundsInsulinFloor`,
`limitsAllowTempBasal`, `commandIsStopPath`, `historySyncId`, …) call straight into
`pump-shared`; they exist so Kotlin and Swift cannot answer the same question differently.

## CGM

A CGM driver registers exactly the way a pump does: a Cargo feature, a line in `DRIVERS` in
`src/registry.rs`, and nothing else. `DriverInfo.kind` is already `Kind.cgm`, and
`matchAdvertisement` does not care what kind of device it found.

One piece is missing, and it is upstream: [Q5](https://github.com/open-source-aid/guidelines/blob/main/14-open-questions.md)
proposes splitting `pump-shared` into `device-shared` + `pump-shared` + `cgm-shared`, and until
that happens `cgm-shared` defines its own `CgmEngine` trait with its own command, outcome and
event types. So a CGM driver needs a `CgmEngineHandle` next to `EngineHandle` — the same ten
methods and the same rules E1-E10 — not a new variant of it. `cgmDrivers()` and
`newCgmEngine()` exist today and return an empty list and `NotImplemented`.

This crate is laid out for that split: `src/host/` is the generic half (`Action`, `Completion`,
`BleOp`, `Slot`, `Class`, `Secret`, `DriverInfo`) and depends on `src/pump/` for exactly one
type, `CommandKind`, which `DriverInfo` itself carries upstream. When `device-shared` lands,
`src/host/` moves with it and `src/pump/` keeps compiling on top.

## Building

```sh
cargo test --all-features
cargo build --release --target aarch64-apple-ios
cargo build --release --target aarch64-apple-ios-sim

cargo run --bin uniffi-bindgen -- generate \
  --library target/aarch64-apple-ios/release/libosaid_bindings.a \
  --language swift --out-dir generated
```

CI builds both iOS targets, type-checks the generated Swift, and assembles an `.xcframework`
from the two static libraries plus the generated Swift as a build artifact. Kotlin is generated
by the same binary with `--language kotlin`; an Android CI job is not written yet.

## Validating a session a host was handed

Some drivers cannot pair a device, and the only route to `EngineState::Paired` is a session
blob produced elsewhere and written to `Slot::Session` by the host. Writing it needs nothing
from this crate — the store and the keystore are the host's — but checking it first does, and a
host must not grow a second parser for a secret format.

`session.rs` is that, and only that: `ypsoInspectSession(blob)` calls `pump_ypso::inspect_session`
and returns a `SessionSummary` (address, serial, version, keyless, credential codes) or a
`SessionBlobError` saying which check failed. Pure — no BLE, no state, nothing written. No key,
no password and no part of the blob is in the answer or in any error, so a host can show the
reason to a user and log it.

## Gaps

What is real: every record and enum above, both conversion directions wherever a value travels
both ways, the registry, `EngineHandle` (all twelve methods), `SimDeviceHandle`, session
validation, and the host loop end to end in process against `YpsoSim`.

What is not:

1. **CGM.** `cgmDrivers()` returns an empty list; `newCgmEngine()` returns
   `BindingError.notImplemented`. Needs `CgmEngineHandle`, and ideally Q5 settled first.
2. **Minting a pairing.** Closed on the host's side, open on the device's. A host that is
   *handed* a session can now use it: `ypsoInspectSession` validates the blob (`session.rs`,
   the driver's own parser — this crate does not parse it), and the host wraps it and writes
   `Slot::Session` itself, after which a fresh engine loads it and reports `Paired`. There is
   still no *encoder* here and there should not be: whoever can mint a pairing holds the
   device's server key, and `pump-ypso/.docs/pairing.md` lists what `Command::Pair` would need
   before it could be declared.
3. **`Unrecognized` arms.** Every data enum has one, because the shared enums are
   `#[non_exhaustive]`: a driver newer than these bindings reports a variant, and the host sees
   `Unrecognized` with a debug string rather than a crash. Handing one back to an engine is an
   error, not a panic. None of them can be reached today, which is what the coverage gate trips
   over (below).
4. **Kotlin.** Generated by the same binary, but no Android CI job and no Kotlin test harness.
5. **Q1 (`provide_input`), Q13 (non-BLE transports), Q9 (device settings).** None of these are
   in the contract yet, so none of them are here.

Nothing in this crate panics on a value from the host. Anything that cannot cross comes back as
a typed `BindingError`.

### Coverage

The org gate is 100 % of lines, regions and functions (Q4). This crate is at **95 %**, and CI
publishes the number on every run and then runs the gate with `continue-on-error`, so the gap
is visible rather than hidden. The uncovered lines are, almost entirely, the `Unrecognized`
arms of gap 3: they cannot be exercised while the shared enums have no variant these bindings
fail to map. Q4 says exclusions live behind a documented feature; there is no feature that fits
an unreachable match arm, so the group needs to settle how these are excluded (a
`coverage(off)` attribute is the usual answer) before the gate can be green and meaningful.

## Licence

AGPL-3.0-or-later. See `LICENSE` and `NOTICE.md`. Whether an app may link this and ship it
through an app store is [Q8](https://github.com/open-source-aid/guidelines/blob/main/14-open-questions.md)
and is not settled.
