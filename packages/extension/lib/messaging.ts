/**
 * What the popup and the content script may ask the background for.
 *
 * The background is the only part of the extension that holds the native
 * port, so every request funnels through here. The content script is the most
 * exposed surface, so what it is allowed to ask for is deliberately narrower
 * than what the popup can: it can look up matches and request one secret for
 * one fill, and nothing else.
 */

import type { Field, Item, Kind, Match, Secret } from "@nopass/protocol";
import type { CapturedRecord } from "./capture";
import type { SessionState } from "./session";

/** A new entry on its way to the store. Never carries an existing name. */
export interface Draft {
  entry: string;
  fields?: Field[];
  kind?: Kind;
  /** The first line: a password, a card number, absent for an identity. */
  secret?: string;
}

/**
 * A change to an entry that is already there.
 *
 * Only the fields named are touched, and an empty value clears one; every
 * other line of the entry — including a field this build has never heard of —
 * is left where it is by the host (ADR-0009).
 */
export interface Patch {
  entry: string;
  fields?: Field[];
  secret?: string;
}

export type PopupRequest =
  | { kind: "session" }
  | { kind: "unlock"; passphrase: string }
  | { kind: "lock" }
  | { kind: "list" }
  | { kind: "search"; origin: string }
  | { kind: "reveal"; entry: string }
  | { kind: "generate"; length: number; symbols: boolean }
  /*
   * The payload is nested rather than spread. A draft carries the record's
   * own `kind` — `card`, `identity` — and spreading it beside the message's
   * `kind` made the two collide into an impossible type, which is how a save
   * that no longer matched the wire went unnoticed.
   */
  | { kind: "save"; draft: Draft }
  | { kind: "update"; patch: Patch }
  | { kind: "items"; kinds?: Kind[] }
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
  /**
   * The cards and identities on offer, as rows and never as secrets. A
   * content script may ask for these two kinds and no others: they belong to
   * no site, so `search` cannot reach them, and enumerating logins is what
   * this vocabulary exists to prevent.
   */
  | { kind: "wallet" }
  | { kind: "captured"; origin: string; record: CapturedRecord }
  | { kind: "saveCaptured"; entry: string }
  | { kind: "dismissCaptured" };

export type { CapturedRecord } from "./capture";

/**
 * What the background will let the page offer, if anything.
 *
 * Null when there is nothing to ask about: the store is locked, or this login
 * is already in it under this username, and a prompt for either would be noise
 * over a page the user is trying to leave.
 */
export interface SaveOffer {
  /** The line under the title: a username, a masked card, a person. */
  detail?: string;
  host: string;
  /** What the prompt is about, so it can say so. */
  kind: Kind;
  suggestion: string;
}

export type ExtensionRequest = PopupRequest | ContentRequest;

export type ExtensionSuccess =
  | { ok: true; kind: "session"; state: SessionState }
  | { ok: true; kind: "entries"; entries: string[] }
  | { ok: true; kind: "matches"; matches: Match[] }
  | { ok: true; kind: "secret"; secret: Secret }
  | { ok: true; kind: "password"; password: string }
  | { ok: true; kind: "saved"; entry: string }
  | { ok: true; kind: "items"; items: Item[] }
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

/**
 * The offer, pushed at whatever page the tab landed on.
 *
 * A sign-in that posts a form takes its page — and the prompt the reply would
 * have raised — away with it. The background still has the login, so it puts
 * the offer on the page that replaced it instead.
 */
export interface OfferCommand {
  kind: "offerSave";
  offer: SaveOffer;
}

export type BackgroundCommand = FillCommand | OfferCommand;

export function failure(code: string, message: string): ExtensionFailure {
  return { code, message, ok: false };
}
