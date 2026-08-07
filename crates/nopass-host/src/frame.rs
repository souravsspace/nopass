//! Native messaging framing: a `uint32` little-endian byte length followed by
//! that many bytes of UTF-8 JSON, in both directions.

use std::io::{Read, Write};

use anyhow::{bail, Result};
use serde::Serialize;
use serde_json::Value;

/// The header is four bytes, always.
const HEADER_BYTES: usize = 4;

/// The largest frame this host will emit or accept.
///
/// Chrome caps a message sent *to* a host at 1 MiB. The declared length
/// arrives before the body does and is entirely under the peer's control, so
/// it is checked before a single byte is reserved for it.
pub const MAX_FRAME_BYTES: usize = 1024 * 1024;

/// Fill `buf` completely. `Ok(false)` means the stream ended cleanly on the
/// boundary; ending part-way through is an error, not an end.
fn fill(reader: &mut impl Read, buf: &mut [u8]) -> Result<bool> {
    let mut filled = 0;
    while filled < buf.len() {
        match reader.read(&mut buf[filled..])? {
            0 if filled == 0 => return Ok(false),
            0 => bail!(
                "the stream ended {filled} bytes into a {} byte read",
                buf.len()
            ),
            read => filled += read,
        }
    }
    Ok(true)
}

/// Read one frame, or `None` when the browser has closed the pipe.
pub fn read_frame(reader: &mut impl Read) -> Result<Option<Value>> {
    let mut header = [0u8; HEADER_BYTES];
    if !fill(reader, &mut header)? {
        return Ok(None);
    }

    let length = u32::from_le_bytes(header) as usize;
    if length > MAX_FRAME_BYTES {
        bail!("frame too large: {length} bytes declared, cap is {MAX_FRAME_BYTES}");
    }

    let mut body = vec![0u8; length];
    if !fill(reader, &mut body)? {
        bail!("the stream ended before the {length} byte body arrived");
    }
    Ok(Some(serde_json::from_slice(&body)?))
}

/// Write one frame and flush it: the browser is waiting on this pipe.
pub fn write_frame(writer: &mut impl Write, value: &impl Serialize) -> Result<()> {
    let body = serde_json::to_vec(value)?;
    if body.len() > MAX_FRAME_BYTES {
        bail!(
            "frame too large: {} bytes, cap is {MAX_FRAME_BYTES}",
            body.len()
        );
    }

    writer.write_all(&(body.len() as u32).to_le_bytes())?;
    writer.write_all(&body)?;
    writer.flush()?;
    Ok(())
}
