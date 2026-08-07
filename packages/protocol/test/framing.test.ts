import { describe, expect, it } from "vitest";
import { createFrameDecoder, encodeFrame, MAX_FRAME_BYTES } from "../src/index";

const TOO_LARGE = /too large/i;

const readLength = (frame: Uint8Array): number =>
  new DataView(frame.buffer, frame.byteOffset, 4).getUint32(0, true);

describe("encodeFrame", () => {
  it("prefixes the payload with its byte length, little-endian", () => {
    const frame = encodeFrame({ id: 1, verb: "status" });
    const body = new TextDecoder().decode(frame.subarray(4));

    expect(readLength(frame)).toBe(body.length);
    expect(JSON.parse(body)).toEqual({ id: 1, verb: "status" });
  });

  it("counts bytes, not characters", () => {
    // "é" is two bytes in UTF-8; a character count would under-declare the
    // frame and desynchronise the stream for every message after it.
    const frame = encodeFrame({ entry: "café" });

    expect(readLength(frame)).toBe(frame.byteLength - 4);
    expect(readLength(frame)).toBeGreaterThan(
      JSON.stringify({ entry: "café" }).length - 1
    );
  });

  it("refuses to emit a frame larger than the cap", () => {
    expect(() => encodeFrame({ pad: "x".repeat(MAX_FRAME_BYTES) })).toThrow(
      TOO_LARGE
    );
  });
});

describe("createFrameDecoder", () => {
  it("returns nothing until a whole frame has arrived", () => {
    const decoder = createFrameDecoder();
    const frame = encodeFrame({ id: 1, verb: "lock" });

    expect(decoder.push(frame.subarray(0, 3))).toEqual([]);
    expect(decoder.push(frame.subarray(3, 6))).toEqual([]);
    expect(decoder.push(frame.subarray(6))).toEqual([{ id: 1, verb: "lock" }]);
  });

  it("splits several frames delivered in one chunk", () => {
    const decoder = createFrameDecoder();
    const first = encodeFrame({ id: 1, verb: "lock" });
    const second = encodeFrame({ id: 2, verb: "list" });
    const both = new Uint8Array(first.byteLength + second.byteLength);
    both.set(first, 0);
    both.set(second, first.byteLength);

    expect(decoder.push(both)).toEqual([
      { id: 1, verb: "lock" },
      { id: 2, verb: "list" },
    ]);
  });

  it("survives a byte-at-a-time stream", () => {
    const decoder = createFrameDecoder();
    const frame = encodeFrame({ id: 9, origin: "https://a.b", verb: "search" });
    const seen: unknown[] = [];

    for (const byte of frame) {
      seen.push(...decoder.push(Uint8Array.of(byte)));
    }

    expect(seen).toEqual([{ id: 9, origin: "https://a.b", verb: "search" }]);
  });

  it("rejects a declared length above the cap without allocating it", () => {
    const decoder = createFrameDecoder();
    const header = new Uint8Array(4);
    new DataView(header.buffer).setUint32(0, MAX_FRAME_BYTES + 1, true);

    expect(() => decoder.push(header)).toThrow(TOO_LARGE);
  });

  it("rejects a frame whose body is not JSON", () => {
    const decoder = createFrameDecoder();
    const body = new TextEncoder().encode("not json");
    const frame = new Uint8Array(4 + body.byteLength);
    new DataView(frame.buffer).setUint32(0, body.byteLength, true);
    frame.set(body, 4);

    expect(() => decoder.push(frame)).toThrow();
  });
});
