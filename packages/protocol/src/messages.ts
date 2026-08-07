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
 * Requests. Only non-mutating verbs exist: there is deliberately no schema for
 * `insert`, `edit`, `rm`, `mv` or `cp`, so a write cannot be expressed on this
 * wire at all (ADR-0002).
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
]);

/**
 * Machine-readable failure reasons. The extension branches on the code; the
 * message is for the log, not for the UI.
 */
export const errorCodeSchema = z.enum([
  "bad_request",
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
