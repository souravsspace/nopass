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
 * absence of `save`, so a page can never put anything into the store either
 * (ADR-0006).
 */
export type ContentRequest =
  | { kind: "session" }
  | { kind: "matches"; origin: string }
  | { kind: "fillHere"; entry: string };

export type ExtensionRequest = PopupRequest | ContentRequest;

export type ExtensionSuccess =
  | { ok: true; kind: "session"; state: SessionState }
  | { ok: true; kind: "entries"; entries: string[] }
  | { ok: true; kind: "matches"; matches: Match[] }
  | { ok: true; kind: "secret"; secret: Secret }
  | { ok: true; kind: "password"; password: string }
  | { ok: true; kind: "saved"; entry: string }
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
