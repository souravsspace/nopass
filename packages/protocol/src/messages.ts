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
export const PROTOCOL_VERSION = 2;

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
 * What an entry holds. Absent means `login`, which is what every entry
 * written before nopass knew about kinds was.
 */
export const kindSchema = z.enum(["login", "card", "identity", "passkey"]);

/**
 * A field key, as the host spells it.
 *
 * The wire vocabulary is canonical: the host resolves `email:` and `user:`
 * into `username` on the way out, and writes back whichever spelling the
 * entry already used, so neither side needs the other's alias table. A bare
 * token keeps a key from being a sentence, a colon, or a second line.
 */
const FIELD_KEY = /^[a-z][a-z0-9-]*$/;
const fieldKey = z
  .string()
  .regex(FIELD_KEY, "a field key is lower-case letters, digits and hyphens")
  // `type:` is the line that names the kind, and the kind travels in its own
  // slot. A field free to write it could turn a login into a card.
  .refine((key) => key !== "type", { message: "the kind is not a field" });

/** One `key: value` line of an entry, in the canonical spelling. */
export const fieldSchema = z.object({ key: fieldKey, value: oneLine });

/** Kinds whose first line is a secret; an identity has none. */
const SECRET_BEARING = new Set(["login", "card", "passkey"]);

/**
 * Requests.
 *
 * Two mutating verbs exist. `insert` can only ever create, and `update` can
 * only ever rewrite an entry that is already there, field by field — there is
 * still no schema for `rm`, `mv` or `cp`, so nothing on this wire can move or
 * destroy what is in the store (ADR-0006, ADR-0009).
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
    /** Absent asks for every kind. */
    kinds: z.array(kindSchema).optional(),
    verb: z.literal("items"),
  }),
  z
    .object({
      entry: entryName,
      fields: z.array(fieldSchema).optional(),
      id: requestId,
      kind: kindSchema.optional(),
      secret: oneLine.optional(),
      verb: z.literal("insert"),
    })
    .refine(
      (request) =>
        !SECRET_BEARING.has(request.kind ?? "login") ||
        (request.secret ?? "").length > 0,
      { message: "this kind of entry needs a secret on its first line" }
    ),
  z
    .object({
      entry: entryName,
      fields: z.array(fieldSchema).optional(),
      id: requestId,
      secret: oneLine.optional(),
      verb: z.literal("update"),
    })
    // An update that names nothing would decrypt and rewrite an entry to
    // leave it exactly as it was. Whatever the caller meant, it was not this.
    .refine(
      (request) =>
        request.secret !== undefined || (request.fields?.length ?? 0) > 0,
      { message: "an update must change something" }
    ),
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
  kind: kindSchema,
  name: entryName,
  url: z.string().optional(),
  username: z.string().optional(),
});

/**
 * A row for an entry that is not offered by origin — a card, an identity.
 *
 * `hint` is what the row reads: a masked card tail, a person's name. It is
 * built by the host precisely so that offering the row costs no secret.
 */
export const itemSchema = z
  .object({
    hint: z.string().optional(),
    kind: kindSchema,
    name: entryName,
  })
  .strict();

/** One entry, handed over for a single fill or for the entry screen. */
export const secretSchema = z.object({
  fields: z.array(fieldSchema),
  kind: kindSchema,
  name: entryName,
  /** The first line: a password, a card number, or empty for an identity. */
  secret: z.string(),
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
  z.object({
    id: requestId,
    items: z.array(itemSchema),
    ok: z.literal(true),
    verb: z.literal("items"),
  }),
  // The name back and nothing else: the popup asked for this write, so it
  // already holds everything a fuller reply could tell it.
  z.object({
    entry: entryName,
    id: requestId,
    ok: z.literal(true),
    verb: z.literal("insert"),
  }),
  z.object({
    entry: entryName,
    id: requestId,
    ok: z.literal(true),
    verb: z.literal("update"),
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
export type Item = z.infer<typeof itemSchema>;
export type Field = z.infer<typeof fieldSchema>;
export type Kind = z.infer<typeof kindSchema>;
export type Secret = z.infer<typeof secretSchema>;
export type Verb = Request["verb"];

/** Narrow a response to the reply for a given verb. */
export type ResponseFor<V extends Verb> = Extract<SuccessResponse, { verb: V }>;
