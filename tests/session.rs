//! The session validator, across the ABI shape a host sees.
//!
//! The blob is built here, in a test, on purpose: building a session is not
//! something this crate does or exposes. Whoever can mint a pairing holds the
//! device's server key; a host is only ever handed the result.

use osaid_bindings::session::{ypso_inspect_session, SessionBlobError};

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &byte in bytes {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xEDB8_8320 & (crc & 1).wrapping_neg());
        }
    }
    !crc
}

fn blob(edit: impl FnOnce(&mut Vec<u8>)) -> Vec<u8> {
    let mut body = b"YB".to_vec();
    body.extend([1, 0, 0xEC, 0x2A, 0xF0, 1, 2, 3]);
    body.extend(10_155_555u32.to_le_bytes());
    body.extend([0x42; 32]);
    body.extend((-1i64).to_le_bytes());
    body.extend((-1i64).to_le_bytes());
    body.extend(0u32.to_le_bytes());
    body.extend(0i64.to_le_bytes());
    body.push(0);
    body.extend([7; 16]);
    body.extend([1, 2, b'O', b'2']);
    body.extend([0x5A; 16]);
    edit(&mut body);
    body.extend(crc32(&body).to_le_bytes());
    body
}

#[test]
fn a_valid_blob_identifies_its_pump_and_nothing_else() {
    let summary = ypso_inspect_session(blob(|_| {})).expect("a valid session blob");
    assert_eq!(summary.address, vec![0xEC, 0x2A, 0xF0, 1, 2, 3]);
    assert_eq!(summary.serial, 10_155_555);
    assert_eq!(summary.version, 1);
    assert!(!summary.keyless);
    assert_eq!(summary.credential_codes, vec![b'O']);

    // The key and the password are in the blob and not in the answer.
    let rendered = format!("{summary:?}");
    assert!(!rendered.contains("66, 66"), "{rendered}"); // 0x42, the key
    assert!(!rendered.contains("90, 90"), "{rendered}"); // 0x5A, the password
}

#[test]
fn each_refusal_says_which_check_failed() {
    let mut short = blob(|_| {});
    short.truncate(50);
    let mut corrupt = blob(|_| {});
    corrupt[95] ^= 1;

    for (bytes, expected) in [
        (vec![], SessionBlobError::NotASession),
        (
            b"not a pairing at all".to_vec(),
            SessionBlobError::NotASession,
        ),
        (short, SessionBlobError::Truncated),
        (corrupt, SessionBlobError::BadChecksum),
        (
            blob(|b| b[2] = 2),
            SessionBlobError::UnsupportedVersion { version: 2 },
        ),
        (blob(|b| b[91] = 2), SessionBlobError::MalformedCredentials),
        (
            blob(|b| b[75..91].copy_from_slice(&[0; 16])),
            SessionBlobError::MalformedBody,
        ),
    ] {
        assert_eq!(ypso_inspect_session(bytes), Err(expected));
    }
}

#[test]
fn no_refusal_quotes_the_blob() {
    let bytes = blob(|_| {});
    let mut corrupt = bytes.clone();
    corrupt[95] ^= 1;
    for candidate in [corrupt, blob(|b| b[2] = 2), blob(|b| b[91] = 2)] {
        let error = ypso_inspect_session(candidate.clone()).unwrap_err();
        let rendered = format!("{error} {error:?}");
        for window in candidate.windows(4) {
            let hex: String = window.iter().map(|b| format!("{b:02x}")).collect();
            assert!(!rendered.to_lowercase().contains(&hex), "{rendered}");
        }
    }
}
