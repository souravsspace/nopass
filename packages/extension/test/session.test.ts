import { describe, expect, it } from "vitest";
import type { SessionState } from "../lib/session";
import { canFill, initialSession, reduce } from "../lib/session";

const connecting: SessionState = { status: "connecting" };

describe("the session machine", () => {
  it("starts out connecting, because nothing is known yet", () => {
    expect(initialSession()).toEqual(connecting);
  });

  it("learns from hello that there is no store to open", () => {
    const next = reduce(connecting, { store: "missing", type: "hello" });
    expect(next).toEqual({ status: "no-store" });
  });

  it("treats a store it has not yet asked about as locked", () => {
    // Assuming unlocked would show the entry list for a moment before the
    // truth arrived. Assuming locked is the safe direction to be wrong in.
    expect(reduce(connecting, { store: "ready", type: "hello" })).toEqual({
      status: "locked",
    });
  });

  it("follows the host's own lock state", () => {
    const unlocked = reduce(connecting, {
      expiresIn: 300,
      state: "unlocked",
      type: "state",
    });
    expect(unlocked).toEqual({ expiresIn: 300, status: "unlocked" });

    expect(
      reduce(unlocked, { expiresIn: 0, state: "locked", type: "state" })
    ).toEqual({
      status: "locked",
    });
  });

  it("does not let a stale state report resurrect a missing store", () => {
    const missing = reduce(connecting, { store: "missing", type: "hello" });
    expect(
      reduce(missing, { expiresIn: 60, state: "unlocked", type: "state" })
    ).toEqual(missing);
  });

  it("reads a locked failure as locked rather than as breakage", () => {
    const unlocked: SessionState = { expiresIn: 10, status: "unlocked" };
    expect(
      reduce(unlocked, { code: "locked", message: "expired", type: "failed" })
    ).toEqual({
      status: "locked",
    });
  });

  it("reads a missing store out of a failure too", () => {
    expect(
      reduce(connecting, {
        code: "store_missing",
        message: "no store",
        type: "failed",
      })
    ).toEqual({ status: "no-store" });
  });

  it("reports a host that is not installed as something the user must fix", () => {
    for (const code of ["unavailable", "timeout"] as const) {
      const next = reduce(connecting, {
        code,
        message: "boom",
        type: "failed",
      });
      expect(next.status).toBe("unavailable");
    }
  });

  it("goes back to connecting when the host exits", () => {
    const unlocked: SessionState = { expiresIn: 300, status: "unlocked" };
    expect(reduce(unlocked, { type: "disconnected" })).toEqual(connecting);
  });

  it("ignores an error it has no opinion about rather than locking the UI up", () => {
    const unlocked: SessionState = { expiresIn: 300, status: "unlocked" };
    expect(
      reduce(unlocked, { code: "not_found", message: "gone", type: "failed" })
    ).toEqual(unlocked);
  });

  it("only offers to fill when the store is actually open", () => {
    expect(canFill({ expiresIn: 1, status: "unlocked" })).toBe(true);
    for (const state of [
      connecting,
      { status: "locked" } as const,
      { status: "no-store" } as const,
      { message: "x", status: "unavailable" } as const,
    ]) {
      expect(canFill(state)).toBe(false);
    }
  });
});
