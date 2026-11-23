//! `cargo run --bin uniffi-bindgen -- generate --library <lib> --language swift`
//!
//! UniFFI's own CLI, built against the exact UniFFI version this crate links,
//! so the generated Swift and the scaffolding can never drift apart.

fn main() {
    uniffi::uniffi_bindgen_main();
}
