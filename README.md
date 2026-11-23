# bindings

The FFI layer that exposes the open-source-aid sans-IO device drivers to Swift and Kotlin, built
with [UniFFI](https://github.com/mozilla/uniffi-rs). One Rust library, statically linked, carrying
every driver the app ships.

This is the **Bindings** piece of [guideline 13](https://github.com/open-source-aid/guidelines/blob/main/13-app-integration.md).
The host runtime and the app adapter live elsewhere.

Status: skeleton. Builds, tested, generated Swift compiles, host loop runs in process against the
driver's own simulator. See [Gaps](#gaps).

## One serial executor per engine

**An engine is not `Sync` and MUST NOT be called concurrently.** One `EngineHandle`, one serial
queue, for its whole life:

```swift
let queue = DispatchQueue(label: "pump.\(serial)")

queue.async {
    while let event = engine.pollEvent() { adapter.on(event) }
    guard let action = engine.pollAction() else { return }
    let completion = host.perform(action)
    try engine.handleCompletion(id: actionId(action: action), completion: completion)
}
```

`EngineHandle` holds a mutex because a UniFFI object is shared as `Arc<T>`. That makes a
concurrent call *safe*, not *correct*: a second queue interleaving `submit` and `pollAction`
breaks rule E1 and the engine reports `CompletionError`. Seat belt, not permission.

## Features

```toml
bindings = { git = "https://github.com/open-source-aid/bindings", branch = "main",
             default-features = false, features = ["driver-ypso", "delivery"] }
```

| Feature | Default | Meaning |
|---|---|---|
| `driver-ypso` | on | Link the YpsoPump driver. One per driver. |
| `delivery` | **off** | Commands that deliver insulin. Stop paths are never behind it. |
| `sim` | **off** | Device simulators. MUST be off in app builds. |
| `ypso-opn` | off | YpsoPump without CryptoService. |
| `hardware` | off | Reserved for the coverage-gate exclusion (Q4). Empty today. |

What was compiled in is visible at run time: `info.features`, `info.commands`, `info.gatt`,
`info.slots`, `info.compatible`.

## From Swift

```swift
let found = matchAdvertisement(advertisement: advertisement)
guard let match = found.first, let driver = driverById(id: match.driverId) else { return }
let engine = try driver.newEngine(variantId: match.variant.id,
                                  config: Config(address: address, utcOffsetSecs: 7200))
let id = try engine.submit(command: .bolus(milliunits: 2_500))
```

Rounding stays in Rust so both platforms round identically: `boundsInsulinFloor` (Loop, Trio),
`boundsInsulinNearest` (AAPS). The ABI carries milliunits, milliunits per hour, whole percent and
whole seconds — never a float. Unit conversion is the adapter's job, at the edge.

With `sim` on, `ypsoSimulator(key:password:)` answers `BleOp` in process — the driver's own
simulator, not a second implementation.

## Layout

```
src/
├── error.rs        BindingError: the one error that crosses
├── host/           the generic half (follows `device-shared` when Q5 lands)
│   action.rs  ble.rs  driver.rs  secret.rs
├── pump/           the pump half
│   command.rs  event.rs  history.rs  outcome.rs  status.rs  units.rs
├── engine.rs       EngineHandle
├── registry.rs     drivers(), driverById(), matchAdvertisement()
├── session.rs      ypsoInspectSession(): validate a session the host was handed
└── sim.rs          SimDeviceHandle (feature `sim`)
```

Handles replace what cannot cross (`&'static DriverInfo`, `Box<dyn Engine>`); `Bounds<T>` is
monomorphised per unit; newtypes become their raw integers; `Secret` crosses as bytes and stays
`Zeroizing` on the Rust side. **No logic is added** — no protocol, no retry or reconnect policy,
no conversion beyond repacking. The exported helpers call straight into `pump-shared` so Kotlin
and Swift cannot answer the same question differently.

A CGM driver registers the same way a pump does: a feature and a line in `DRIVERS`.

## Building

```sh
cargo test --all-features
cargo build --release --target aarch64-apple-ios
cargo build --release --target aarch64-apple-ios-sim

cargo run --bin uniffi-bindgen -- generate \
  --library target/aarch64-apple-ios/release/libosaid_bindings.a \
  --language swift --out-dir generated
```

CI builds both iOS targets, type-checks the generated Swift and assembles an `.xcframework`.
Kotlin generates with `--language kotlin`; no Android CI job yet.

## Gaps

1. **CGM.** `cgmDrivers()` is empty, `newCgmEngine()` returns `NotImplemented`. Needs a
   `CgmEngineHandle`, ideally after Q5.
2. **Minting a pairing.** A host that is *handed* a session can use it: `ypsoInspectSession`
   validates the blob with the driver's own parser, then the host wraps it and writes
   `Slot::Session` itself. There is no encoder here and should not be — whoever can mint a
   pairing holds the device's server key. See `pump-ypso/.docs/pairing.md`.
3. **`Unrecognized` arms.** Every data enum has one, because the shared enums are
   `#[non_exhaustive]`: a newer driver reports a variant the host sees as `Unrecognized` rather
   than a crash. Unreachable today, which is what the coverage gate trips over.
4. **Kotlin.** Generates, but no CI job and no test harness.
5. **Q1 `provide_input`, Q13 non-BLE transports, Q9 device settings** — not in the contract yet.

Nothing here panics on a value from the host; anything that cannot cross is a typed
`BindingError`.

**Coverage** is at 95 % against the org's 100 % gate (Q4). The uncovered lines are almost all the
`Unrecognized` arms above. CI publishes the number and runs the gate with `continue-on-error`, so
the gap is visible rather than hidden; the group needs to settle how an unreachable arm is
excluded (`coverage(off)` is the usual answer).

## Licence

AGPL-3.0-or-later. See `LICENSE` and `NOTICE.md`. Whether an app may link this and ship it through
an app store is [Q8](https://github.com/open-source-aid/guidelines/blob/main/14-open-questions.md),
unsettled.
