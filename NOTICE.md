# Notices: bindings

bindings is licensed under the GNU Affero General Public License v3.0 or later (`LICENSE`).
Draft for review; not legal advice.

bindings contains no device protocol code and is not derived from any existing app driver. It
exposes the open-source-aid crates across an FFI boundary and nothing else.

It links, via Cargo:

- `pump-shared` and `pump-ypso` (open-source-aid, AGPL-3.0-or-later), and through `pump-ypso`
  the crates listed in that repository's `NOTICE.md`;
- `uniffi` (Mozilla, MPL-2.0), which also generates the Swift and Kotlin source this crate
  ships. Generated bindings carry no additional restriction beyond this repository's licence;
- `zeroize` (Apache-2.0 OR MIT);
- `thiserror` (Apache-2.0 OR MIT).

## Distribution

Open: Q8. Whether linking an AGPL-3.0-or-later static library into AndroidAPS, Trio or Loop,
and distributing the result through the App Store or Google Play, is compatible with each app's
own licence and with those stores' terms is **not settled**. This file records the question; it
does not answer it. A proper legal opinion is needed before any app ships this crate.

## Trademarks

Device and app names appear only to say which device or app a part works with. They belong to
their owners, and this project is not made, endorsed or supported by any of them.
