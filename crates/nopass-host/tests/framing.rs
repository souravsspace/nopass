//! Framing is the only place the host reads attacker-influenced lengths, so
//! it gets its own suite.

use std::io::{Cursor, Read};

use nopass_host::frame::{read_frame, write_frame, MAX_FRAME_BYTES};
use serde_json::json;

/// A reader that hands over one byte at a time, the way a pipe can.
struct Trickle(Cursor<Vec<u8>>);

impl Read for Trickle {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        self.0.read(&mut buf[..1])
    }
}

fn framed(value: serde_json::Value) -> Vec<u8> {
    let mut out = Vec::new();
    write_frame(&mut out, &value).expect("a small value frames");
    out
}

#[test]
fn a_frame_round_trips() {
    let bytes = framed(json!({ "id": 1, "verb": "status" }));
    let mut reader = Cursor::new(bytes);

    let value = read_frame(&mut reader).expect("reading works").expect("a frame is there");
    assert_eq!(value, json!({ "id": 1, "verb": "status" }));
}

#[test]
fn the_header_declares_bytes_not_characters() {
    let bytes = framed(json!({ "entry": "café" }));
    let declared = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize;

    assert_eq!(declared, bytes.len() - 4);
}

#[test]
fn several_frames_come_back_in_order() {
    let mut bytes = framed(json!({ "id": 1 }));
    bytes.extend(framed(json!({ "id": 2 })));
    let mut reader = Cursor::new(bytes);

    assert_eq!(read_frame(&mut reader).unwrap(), Some(json!({ "id": 1 })));
    assert_eq!(read_frame(&mut reader).unwrap(), Some(json!({ "id": 2 })));
    assert_eq!(read_frame(&mut reader).unwrap(), None);
}

#[test]
fn a_frame_delivered_one_byte_at_a_time_still_arrives() {
    let mut reader = Trickle(Cursor::new(framed(json!({ "id": 9, "verb": "lock" }))));

    let value = read_frame(&mut reader).expect("reading works").expect("a frame is there");
    assert_eq!(value, json!({ "id": 9, "verb": "lock" }));
}

#[test]
fn a_clean_end_of_stream_is_not_an_error() {
    let mut reader = Cursor::new(Vec::new());
    assert_eq!(read_frame(&mut reader).expect("EOF is not an error"), None);
}

#[test]
fn a_truncated_frame_is_an_error() {
    let mut bytes = framed(json!({ "id": 1, "verb": "status" }));
    bytes.truncate(bytes.len() - 3);
    let mut reader = Cursor::new(bytes);

    assert!(read_frame(&mut reader).is_err());
}

#[test]
fn an_oversized_declared_length_is_refused_before_it_is_allocated() {
    // Only the header is supplied. If the cap were checked after allocating,
    // this would reserve a gigabyte and then block waiting for a body that is
    // never coming.
    let mut bytes = ((MAX_FRAME_BYTES + 1) as u32).to_le_bytes().to_vec();
    bytes.extend_from_slice(b"{}");
    let mut reader = Cursor::new(bytes);

    let error = read_frame(&mut reader).expect_err("the cap is enforced");
    assert!(format!("{error}").contains("too large"), "{error}");
}

#[test]
fn a_body_that_is_not_json_is_an_error() {
    let body = b"not json";
    let mut bytes = (body.len() as u32).to_le_bytes().to_vec();
    bytes.extend_from_slice(body);
    let mut reader = Cursor::new(bytes);

    assert!(read_frame(&mut reader).is_err());
}
