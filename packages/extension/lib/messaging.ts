/**
 * What the popup and the content script may ask the background for.
 *
 * The background is the only part of the extension that holds the native
 * port, so every request funnels through here. The content script is the most
 * exposed surface, so what it is allowed to ask for is deliberately narrower
 * than what the popup can: it can look up matches and request one secret for
 * one fill, and nothing else.
 */

import type { Match, Secret } from "@nopass/protocol";
import type { SessionState } from "./session";

/** A new login on its way to the store. Never carries an existing name. */
export interface Draft {
  entry: string;
  password: string;
  url?: string;
  username?: string;
}

export type PopupRequest =
  | { kind: "session" }
  | { kind: "unlock"; passphrase: string }
  | { kind: "lock" }
  | { kind: "list" }
  | { kind: "search"; origin: string }
  | { kind: "reveal"; entry: string }
  | { kind: "generate"; length: number; symbols: boolean }
  | ({ kind: "save" } & Draft)
  | { kind: "fill"; entry: string; tabId: number };

/**
 * The content script's vocabulary. Note the absence of `reveal`: a page's
 * script may cause a fill, but may never be handed a secret to read — and the
 * absence of `save`, which stays a popup-only word: what a content script may
 * do is offer a login the user just typed, and then name one the background is
 * already holding (ADR-0006, ADR-0007).
 */
export type ContentRequest =
  | { kind: "session" }
  | { kind: "matches"; origin: string }
  | { kind: "fillHere"; entry: string }
  | ({ kind: "captured"; origin: string } & Captured)
  | { kind: "saveCaptured"; entry: string }
  | { kind: "dismissCaptured" };

/** A login read off a page as it was submitted. */
export interface Captured {
  password: string;
  username?: string;
}

/**
 * What the background will let the page offer, if anything.
 *
 * Null when there is nothing to ask about: the store is locked, or this login
 * is already in it under this username, and a prompt for either would be noise
 * over a page the user is trying to leave.
 */
export interface SaveOffer {
  host: string;
  suggestion: string;
  username?: string;
}

export type ExtensionRequest = PopupRequest | ContentRequest;

export type ExtensionSuccess =
  | { ok: true; kind: "session"; state: SessionState }
  | { ok: true; kind: "entries"; entries: string[] }
  | { ok: true; kind: "matches"; matches: Match[] }
  | { ok: true; kind: "secret"; secret: Secret }
  | { ok: true; kind: "password"; password: string }
  | { ok: true; kind: "saved"; entry: string }
  | { ok: true; kind: "offer"; offer: SaveOffer | null }
  | { ok: true; kind: "done" };

export interface ExtensionFailure {
  code: string;
  message: string;
  ok: false;
}

export type ExtensionReply = ExtensionSuccess | ExtensionFailure;

/** What the background pushes at a content script to make it fill. */
export interface FillCommand {
  kind: "fill";
  secret: Secret;
}

export function failure(code: string, message: string): ExtensionFailure {
  return { code, message, ok: false };
}
