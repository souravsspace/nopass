/**
 * Native messaging framing: a `uint32` little-endian byte length followed by
 * that many bytes of UTF-8 JSON, in both directions.
 *
 * The browser does this for the extension, so nothing in `packages/extension`
 * needs it. The e2e stub host speaks the wire directly, and having one
 * implementation here means the stub cannot quietly disagree with the real
 * host about what a frame is.
 */

/** The header is four bytes, always. */
const HEADER_BYTES = 4;

/**
 * The largest frame either side will emit or accept. Chrome caps a message
 * sent *to* a host at 1 MiB, and a declared length is attacker-controlled, so
 * this is checked before anything is allocated for it.
 */
export const MAX_FRAME_BYTES = 1024 * 1024;

/** Serialise `value` into one length-prefixed frame. */
export function encodeFrame(value: unknown): Uint8Array {
  const body = new TextEncoder().encode(JSON.stringify(value));
  if (body.byteLength > MAX_FRAME_BYTES) {
    throw new Error(
      `frame too large: ${body.byteLength} bytes exceeds the ${MAX_FRAME_BYTES} byte cap`
    );
  }

  const frame = new Uint8Array(HEADER_BYTES + body.byteLength);
  new DataView(frame.buffer).setUint32(0, body.byteLength, true);
  frame.set(body, HEADER_BYTES);
  return frame;
}

export interface FrameDecoder {
  /** Feed bytes in; get back whichever frames completed. */
  push: (chunk: Uint8Array) => unknown[];
}

/**
 * A decoder for a stream that respects no message boundaries: a chunk may
 * carry half a frame, three frames, or one byte.
 */
export function createFrameDecoder(): FrameDecoder {
  let pending = new Uint8Array(0);

  return {
    push(chunk: Uint8Array): unknown[] {
      const grown = new Uint8Array(pending.byteLength + chunk.byteLength);
      grown.set(pending, 0);
      grown.set(chunk, pending.byteLength);
      pending = grown;

      const frames: unknown[] = [];
      while (pending.byteLength >= HEADER_BYTES) {
        const length = new DataView(
          pending.buffer,
          pending.byteOffset,
          HEADER_BYTES
        ).getUint32(0, true);

        if (length > MAX_FRAME_BYTES) {
          throw new Error(
            `frame too large: ${length} bytes declared, cap is ${MAX_FRAME_BYTES}`
          );
        }
        if (pending.byteLength < HEADER_BYTES + length) {
          break;
        }

        const body = pending.subarray(HEADER_BYTES, HEADER_BYTES + length);
        frames.push(JSON.parse(new TextDecoder().decode(body)));
        pending = pending.slice(HEADER_BYTES + length);
      }

      return frames;
    },
  };
}
