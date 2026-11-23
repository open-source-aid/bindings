//! `EngineHandle`: `Box<dyn Engine>` as an object the host can hold.
//!
//! # One serial executor per engine
//!
//! **An engine is not `Sync` and MUST NOT be called concurrently**
//! (guideline 13). The host loop for one engine runs on one serial queue or
//! one thread, and every method below is called from it:
//!
//! ```swift
//! let queue = DispatchQueue(label: "pump.\(serial)")   // serial, one per engine
//! queue.async {
//!     while let event = engine.pollEvent() { adapter.onEvent(event) }
//!     guard let action = engine.pollAction() else { return }
//!     let completion = host.perform(action)            // may be async on the platform side
//!     try engine.handleCompletion(id: actionId(action: action), completion: completion)
//! }
//! ```
//!
//! The lock below exists because UniFFI objects are shared as `Arc<T>` and so
//! must be `Send + Sync`. It makes a concurrent call *safe* — it cannot make
//! it *correct*: a second queue interleaving `submit` and `pollAction` will
//! still break rule E1 (at most one action outstanding) in ways the engine
//! reports as `CompletionError`, not as a crash. Treat the lock as a seat
//! belt, not as permission.
//!
//! Every method returns promptly (E2). None of them blocks, sleeps or spawns
//! anything.

use std::sync::Mutex;

use pump_shared::{Command as PsCommand, Completion as PsCompletion};
use pump_shared::{Engine, OpId, Timestamp, UtcTime};

use crate::error::BindingError;
use crate::host::action::{Action, Completion};
use crate::host::ble::Notification;
use crate::host::driver::EngineState;
use crate::pump::command::Command;
use crate::pump::event::Event;
use crate::pump::outcome::Refusal;
use crate::pump::status::TherapyState;

/// The engine, and the promise that only one thread is in it at a time.
struct Serial(Box<dyn Engine>);

// SAFETY: `dyn Engine` carries no `Send` bound, because a driver engine is
// plain state with no thread affinity: it holds no thread-local, no OS handle
// and no `Rc` reachable from its public API, and rule E2 forbids it from
// owning a thread or an executor. It is therefore sound to move between
// threads, which is what a host's serial `DispatchQueue` does between blocks.
// It is *not* sound to touch from two threads at once, and the `Mutex` in
// `EngineHandle` is what rules that out.
unsafe impl Send for Serial {}

impl core::fmt::Debug for EngineHandle {
    /// Never prints engine state: an engine holds session keys and counters,
    /// and `Debug` output ends up in logs (E10).
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("EngineHandle")
    }
}

/// One device's state machine. Create it with
/// [`DriverHandle::new_engine`](crate::registry::DriverHandle::new_engine).
#[derive(uniffi::Object)]
pub struct EngineHandle {
    engine: Mutex<Serial>,
}

impl EngineHandle {
    pub(crate) fn new(engine: Box<dyn Engine>) -> Self {
        Self {
            engine: Mutex::new(Serial(engine)),
        }
    }

    /// The lock is only ever held for the length of one contract call, and no
    /// call below can panic while holding it, so a poisoned lock means the
    /// host called an engine from a thread that died elsewhere. Recovering it
    /// keeps that from turning into a second, unrelated panic across the FFI.
    fn with<T>(&self, f: impl FnOnce(&mut dyn Engine) -> T) -> T {
        let mut guard = self
            .engine
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        f(guard.0.as_mut())
    }
}

#[uniffi::export]
impl EngineHandle {
    /// Queue a command. Returns its id; exactly one `Event::Finished` will
    /// carry that id (E5). Nothing is performed inline beyond producing the
    /// first action (E3).
    pub fn submit(&self, command: Command) -> Result<u32, BindingError> {
        let command = PsCommand::try_from(command)?;
        self.with(|engine| engine.submit(command))
            .map(|id| id.get())
            .map_err(Into::into)
    }

    /// Always accepted, pre-empts the queue, never refused for a missing
    /// feature (E4).
    pub fn stop(&self) -> u32 {
        self.with(|engine| engine.stop()).get()
    }

    /// Hand back the result of exactly the action with this id (E1).
    pub fn handle_completion(&self, id: u32, completion: Completion) -> Result<(), BindingError> {
        let completion = PsCompletion::try_from(completion)?;
        self.with(|engine| engine.handle_completion(OpId::new(id), completion))
            .map_err(Into::into)
    }

    /// The monotonic clock reached `poll_timeout`.
    pub fn handle_timeout(&self, now_millis: u64) {
        self.with(|engine| engine.handle_timeout(Timestamp::from_millis(now_millis)));
    }

    /// Wall-clock time, for pumps that keep their own clock.
    pub fn handle_wall_clock(&self, unix_millis: u64) {
        self.with(|engine| engine.handle_wall_clock(UtcTime::from_unix_millis(unix_millis)));
    }

    /// The link dropped. Anything in flight finishes as `Unknown` (E8).
    pub fn handle_link_lost(&self) {
        self.with(|engine| engine.handle_link_lost());
    }

    /// A notification from a characteristic the driver declared `notify` (E9).
    pub fn handle_notification(&self, notification: Notification) {
        self.with(|engine| engine.handle_notification(notification.into()));
    }

    /// The next thing the host must do, or `nil` when the engine is idle.
    pub fn poll_action(&self) -> Option<Action> {
        self.with(|engine| engine.poll_action()).map(Into::into)
    }

    /// Drain these before every `poll_action`.
    pub fn poll_event(&self) -> Option<Event> {
        self.with(|engine| engine.poll_event()).map(Into::into)
    }

    /// Monotonic milliseconds at which to call `handle_timeout`, if any.
    pub fn poll_timeout(&self) -> Option<u64> {
        self.with(|engine| engine.poll_timeout())
            .map(|timeout| timeout.as_millis())
    }

    pub fn state(&self) -> EngineState {
        self.with(|engine| engine.state()).into()
    }

    /// The therapy state machine's current state.
    pub fn therapy(&self) -> TherapyState {
        self.with(|engine| *engine.therapy()).into()
    }

    /// Why this command would be refused right now, or `nil` if it would be
    /// accepted. Asking costs nothing and sends nothing to the device.
    pub fn guard(&self, command: Command) -> Result<Option<Refusal>, BindingError> {
        let command = PsCommand::try_from(command)?;
        Ok(self
            .with(|engine| engine.guard(&command))
            .err()
            .map(Into::into))
    }
}
