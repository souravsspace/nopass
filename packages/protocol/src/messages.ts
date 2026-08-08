/**
 * The nopass native messaging contract.
 *
 * `fixtures/messages.json` is the source of truth; these schemas and the serde
 * types in `crates/nopass-host` are both tested against it. Changing a shape
 * here without changing the fixture will fail on both sides at once, which is
 * the point.
 */

import { z } from "zod";

/** Bumped whenever a change would break an older extension. */
export const PROTOCOL_VERSION = 1;

/** Bounds on `generate`, so a typo cannot ask the host for a megabyte. */
export const MIN_PASSWORD_LENGTH = 8;
export const MAX_PASSWORD_LENGTH = 1024;

const requestId = z.number().int().nonnegative();
const entryName = z.string().min(1);

/**
 * One line of an entry body, and never more than one.
 *
 * An entry is `password\nkey: value\n…`, so a value carrying a newline could
 * forge a second field — a `url:` line pointing somewhere the user never
 * typed, which is a phishing primitive rather than a formatting bug. Rejected
 * at the edge on both sides of the wire (ADR-0006).
 */
const LINE_BREAK = /[\r\n]/;
const oneLine = z.string().refine((value) => !LINE_BREAK.test(value), {
  message: "must not contain a line break",
});

/**
 * Requests. Only one mutating verb exists, and it can only ever create:
 * there is deliberately no schema for `edit`, `rm`, `mv` or `cp`, so nothing
 * already in the store can be rewritten, moved or destroyed over this wire
 * (ADR-0006, superseding ADR-0002 in that one respect).
 */
export const requestSchema = z.discriminatedUnion("verb", [
  z.object({
    id: requestId,
    verb: z.literal("hello"),
    version: z.number().int().positive(),
  }),
  z.object({ id: requestId, verb: z.literal("status") }),
  z.object({
    id: requestId,
    passphrase: z.string().min(1),
    verb: z.literal("unlock"),
  }),
  z.object({ id: requestId, verb: z.literal("lock") }),
  z.object({ id: requestId, verb: z.literal("list") }),
  z.object({
    id: requestId,
    origin: z.string().min(1),
    verb: z.literal("search"),
  }),
  z.object({ entry: entryName, id: requestId, verb: z.literal("get") }),
  z.object({
    entry: entryName,
    id: requestId,
    password: oneLine.min(1),
    url: oneLine.optional(),
    username: oneLine.optional(),
    verb: z.literal("insert"),
  }),
  z.object({
    id: requestId,
    length: z.number().int().min(MIN_PASSWORD_LENGTH).max(MAX_PASSWORD_LENGTH),
    symbols: z.boolean(),
    verb: z.literal("generate"),
  }),
]);

/** Whether the store exists at all, so the popup can offer `nopass init`. */
const storeState = z.enum(["ready", "missing"]);
const lockState = z.enum(["locked", "unlocked"]);

/** A row in the popup or the inline dropdown. Never carries a secret. */
export const matchSchema = z.object({
  name: entryName,
  url: z.string().optional(),
  username: z.string().optional(),
});

/** One entry's secret, handed over for a single fill. */
export const secretSchema = z.object({
  name: entryName,
  password: z.string(),
  totp: z.string().optional(),
  url: z.string().optional(),
  username: z.string().optional(),
});

const successSchema = z.discriminatedUnion("verb", [
  z.object({
    id: requestId,
    ok: z.literal(true),
    store: storeState,
    verb: z.literal("hello"),
    version: z.number().int().positive(),
  }),
  z.object({
    expiresIn: z.number().int().nonnegative(),
    id: requestId,
    ok: z.literal(true),
    state: lockState,
    verb: z.literal("status"),
  }),
  z.object({
    expiresIn: z.number().int().nonnegative(),
    id: requestId,
    ok: z.literal(true),
    state: lockState,
    verb: z.literal("unlock"),
  }),
  z.object({
    id: requestId,
    ok: z.literal(true),
    state: lockState,
    verb: z.literal("lock"),
  }),
  z.object({
    entries: z.array(entryName),
    id: requestId,
    ok: z.literal(true),
    verb: z.literal("list"),
  }),
  z.object({
    id: requestId,
    matches: z.array(matchSchema),
    ok: z.literal(true),
    verb: z.literal("search"),
  }),
  z.object({
    entry: secretSchema,
    id: requestId,
    ok: z.literal(true),
    verb: z.literal("get"),
  }),
  z.object({
    id: requestId,
    ok: z.literal(true),
    password: z.string(),
    verb: z.literal("generate"),
  }),
  // The name back and nothing else: the popup asked for this write, so it
  // already holds everything a fuller reply could tell it.
  z.object({
    entry: entryName,
    id: requestId,
    ok: z.literal(true),
    verb: z.literal("insert"),
  }),
]);

/**
 * Machine-readable failure reasons. The extension branches on the code; the
 * message is for the log, not for the UI.
 */
export const errorCodeSchema = z.enum([
  "bad_request",
  // A name already taken. Distinct from `bad_request` because the popup
  // answers it by offering another name rather than by saying "that is wrong".
  "exists",
  "internal",
  "locked",
  "not_found",
  "read_only",
  "store_missing",
  "unsupported_verb",
]);

export const errorSchema = z.object({
  error: z.object({ code: errorCodeSchema, message: z.string() }),
  id: requestId,
  ok: z.literal(false),
});

export const responseSchema = z.union([successSchema, errorSchema]);

export type Request = z.infer<typeof requestSchema>;
export type Response = z.infer<typeof responseSchema>;
export type SuccessResponse = z.infer<typeof successSchema>;
export type ErrorResponse = z.infer<typeof errorSchema>;
export type ErrorCode = z.infer<typeof errorCodeSchema>;
export type Match = z.infer<typeof matchSchema>;
export type Secret = z.infer<typeof secretSchema>;
export type Verb = Request["verb"];

/** Narrow a response to the reply for a given verb. */
export type ResponseFor<V extends Verb> = Extract<SuccessResponse, { verb: V }>;
