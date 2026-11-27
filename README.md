# bindings

The FFI layer that exposes the open-source-aid sans-IO device drivers to Swift and Kotlin, built
with [UniFFI](https://github.com/mozilla/uniffi-rs). It is a single Rust library, statically
linked into the app, carrying whichever drivers that app selects at build time.

Its job is narrow on purpose: it **repacks** the driver types so they can cross an FFI boundary,
and does nothing else. No protocol, no retry or reconnect policy, no unit conversion beyond
moving bytes and integers from one side to the other. Every answer it exposes calls straight into
the shared driver crates, so Swift and Kotlin can never answer the same question two different
ways. This is the Bindings piece of the app-integration guideline; the host runtime and the
per-app adapter live in other repositories.

## One serial executor per engine

This is the single rule a host must get right. **An engine is not thread-safe and must never be
called concurrently.** Give each engine one serial queue for its whole life, and drive it from
that queue only:

```swift
let queue = DispatchQueue(label: "device.\(serial)")

queue.async {
    while let event = engine.pollEvent() { adapter.on(event) }
    guard let action = engine.pollAction() else { return }
    let completion = host.perform(action)
    try engine.handleCompletion(id: actionId(action: action), completion: completion)
}
```

An engine handle holds a mutex, because UniFFI shares it as a reference-counted object. That makes
a concurrent call *memory-safe*, not *correct*: a second queue interleaving calls corrupts the
engine's view of what it asked for, and it rejects the completion. Treat the mutex as a seat belt,
not as permission to call from two places.

## Choosing drivers and features

An app enables one Cargo feature per driver it links, plus a few cross-cutting switches. Today a
single driver ships — the YpsoPump — so the table is short; more drivers appear here as one feature
each, and a CGM driver registers exactly the same way a pump does.

| Feature | Default | Meaning |
|---|---|---|
| `driver-ypso` | on | Link the one pump driver available today. One feature per driver. |
| `delivery` | **off** | Enables the commands that deliver insulin. Commands that *stop* delivery are never behind this switch. |
| `sim` | **off** | In-process device simulators for a test harness. Must stay off in app builds. |
| `hardware` | off | Reserved for code that only runs against real hardware; excluded from the coverage gate. |

What was compiled in is readable at run time through `info.features`, `info.commands`,
`info.gatt`, `info.slots` and `info.compatible` — a host never has to guess which drivers or
capabilities a given build carries.

## Using it from Swift

The flow is the same whatever the device: match an advertisement, open the matching driver, create
an engine, submit commands.

```swift
let found = matchAdvertisement(advertisement: advertisement)
guard let match = found.first, let driver = driverById(id: match.driverId) else { return }
let engine = try driver.newEngine(variantId: match.variant.id,
                                  config: Config(address: address, utcOffsetSecs: 7200))
let id = try engine.submit(command: .bolus(milliunits: 2_500))
```

Nothing here panics on a value from the host: anything that cannot cross comes back as a typed
error.

## Units stay integers, rounding stays in Rust

The boundary carries only whole integers — milliunits, milliunits per hour, whole percent, whole
seconds — never a floating-point number. Rounding to what a device actually accepts happens in
Rust, so both platforms round identically (`boundsInsulinFloor` for Loop and Trio,
`boundsInsulinNearest` for AndroidAPS). Converting those integers into display units is the app
adapter's job, at the very edge.

A driver also exposes a *nominal* description of the device family — what the hardware is built
for, readable before any device is connected, for a setup screen. These nominal records are
deliberately separate from the live limits a real dose is checked and rounded against, and nothing
converts between the two: a family ceiling is never mistaken for a live bound.

## Layout

```
src/
├── error.rs        the one error type that crosses the boundary
├── host/           the generic half: actions, BLE ops, driver registry, secrets
├── pump/           the pump half: commands, events, history, outcomes, status, units
├── engine.rs       the engine handle
├── registry.rs     drivers(), driverById(), matchAdvertisement()
├── session.rs      validate a pairing session the host was handed
└── sim.rs          in-process simulator handle (feature `sim`)
```

Handles stand in for anything that cannot physically cross the boundary; secrets cross as bytes and
are zeroized on the Rust side; typed enums are marked non-exhaustive so a newer driver can report a
value an older host sees as "unrecognized" rather than crashing.

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
Kotlin generates with `--language kotlin`; there is no Android CI job yet.

## Status

A working skeleton: it builds, it is tested, the generated Swift compiles, and the host loop runs
in process against a driver's own simulator. Known gaps:

- **CGM support** is stubbed — the registry is pump-only for now.
- **Pairing** can be *used* here but not *minted*: a host that is handed a session can have it
  validated, but creating a new pairing stays with whoever holds the device's keys, by design.
- **Kotlin** generates but has no CI job or test harness yet.
- **Coverage** sits a little under the org's 100 % gate; the uncovered lines are almost all the
  unreachable "unrecognized" arms, and CI reports the number rather than hiding it.

## Licence

AGPL-3.0-or-later. See `LICENSE` and `NOTICE.md`. Whether an app may link this library and ship it
through an app store is an open licensing question, not yet settled.
