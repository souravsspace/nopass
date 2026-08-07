/**
 * What the extension believes about the store right now.
 *
 * A pure reducer, so the popup, the content script and the background can all
 * agree without any of them owning the truth. The host is the authority; this
 * only remembers what it last said (ADR-0005).
 */

import type { ErrorCode } from "@nopass/protocol";
import type { ClientErrorCode } from "./native";

export type SessionState =
  | { status: "connecting" }
  | { status: "unavailable"; message: string }
  | { status: "no-store" }
  | { status: "locked" }
  | { status: "unlocked"; expiresIn: number };

export type SessionEvent =
  | { type: "hello"; store: "ready" | "missing" }
  | { type: "state"; state: "locked" | "unlocked"; expiresIn: number }
  | { type: "failed"; code: ErrorCode | ClientErrorCode; message: string }
  | { type: "disconnected" };

export function initialSession(): SessionState {
  return { status: "connecting" };
}

export function reduce(state: SessionState, event: SessionEvent): SessionState {
  switch (event.type) {
    case "hello":
      // Nothing has been asked about the lock yet. Locked is the safe
      // direction to be wrong in: showing entries and then taking them away
      // is worse than a redundant unlock prompt.
      return event.store === "missing"
        ? { status: "no-store" }
        : { status: "locked" };

    case "state":
      // A store that does not exist cannot be unlocked, whatever a late
      // reply from before `nopass init` might claim.
      if (state.status === "no-store") {
        return state;
      }
      return event.state === "unlocked"
        ? { expiresIn: event.expiresIn, status: "unlocked" }
        : { status: "locked" };

    case "failed":
      return fromFailure(state, event.code, event.message);

    case "disconnected":
      // The host exits when the browser drops the port. The next request
      // starts it again, so this is a pause, not a fault.
      return { status: "connecting" };

    default:
      return state;
  }
}

function fromFailure(
  state: SessionState,
  code: ErrorCode | ClientErrorCode,
  message: string
): SessionState {
  switch (code) {
    case "locked":
      return { status: "locked" };
    case "store_missing":
      return { status: "no-store" };
    case "unavailable":
    case "timeout":
      return { message, status: "unavailable" };
    case "disconnected":
      return { status: "connecting" };
    default:
      // `not_found`, `bad_request` and friends are about one request, not
      // about the session. Tearing the UI down over them would be wrong.
      return state;
  }
}

/** Whether the extension is in a position to hand a secret to a page. */
export function canFill(state: SessionState): boolean {
  return state.status === "unlocked";
}
