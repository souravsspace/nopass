/**
 * Reading an entry the host has handed over.
 *
 * The wire speaks one canonical key per field, resolved by the host from
 * whatever spelling the entry itself uses, so nothing here needs an alias
 * table — only the list of keys each kind is asked for and the words a person
 * should see beside them.
 */

import type { Field, Kind, Secret } from "@nopass/protocol";
import { displayName } from "./dropdown";

/** How much of a card number is ever shown. */
const CARD_TAIL = 4;

/** The fields each kind asks for, in the order a screen should show them. */
const FIELDS: Record<Kind, string[]> = {
  card: ["cardholder", "exp-month", "exp-year", "cvv", "brand", "zip"],
  identity: [
    "given-name",
    "family-name",
    "email",
    "phone",
    "birthday",
    "age",
    "street",
    "street2",
    "city",
    "region",
    "postcode",
    "country",
    "organization",
  ],
  login: ["username", "url", "totp"],
  passkey: ["rp", "user", "user-handle", "credential-id", "alg", "counter"],
};

/** Where a key's own words beat turning the key into a sentence. */
const LABELS: Record<string, string> = {
  "credential-id": "Credential",
  cvv: "Security code",
  "exp-month": "Expiry month",
  "exp-year": "Expiry year",
  "family-name": "Last name",
  "given-name": "First name",
  organization: "Company",
  rp: "Site",
  street2: "Address line 2",
  totp: "One-time code",
  url: "Website",
  "user-handle": "User handle",
  zip: "Postcode",
};

/** The fields `kind` asks for. */
export function fieldsFor(kind: Kind): string[] {
  return FIELDS[kind] ?? [];
}

/** One field's value, or undefined when the entry does not carry it. */
export function read(entry: Secret, key: string): string | undefined {
  return entry.fields.find((field) => field.key === key)?.value;
}

/**
 * `fields` with `key` set to `value`.
 *
 * An empty value is kept rather than dropped: that is how the host is told to
 * clear a field, and dropping it here would silently leave the old one.
 */
export function withField(
  fields: Field[],
  key: string,
  value: string
): Field[] {
  const found = fields.some((field) => field.key === key);
  return found
    ? fields.map((field) => (field.key === key ? { key, value } : field))
    : [...fields, { key, value }];
}

/** A record of values as the wire wants them, minus everything left blank. */
export function toFields(values: Record<string, string | undefined>): Field[] {
  return Object.entries(values)
    .filter(([, value]) => (value ?? "").trim().length > 0)
    .map(([key, value]) => ({ key, value: (value ?? "").trim() }));
}

/** The words for a field, for a label or a placeholder. */
export function labelFor(key: string): string {
  const known = LABELS[key];
  if (known) {
    return known;
  }
  const words = key.replace(/-/g, " ");
  return words.charAt(0).toUpperCase() + words.slice(1);
}

/**
 * What a row for this entry should read.
 *
 * A card is named by its brand and its last four digits and never by its
 * number, which is on screen only on its own screen, behind Show.
 */
export function title(entry: Secret): string {
  switch (entry.kind) {
    case "card": {
      const digits = entry.secret.replace(/\D/g, "");
      const tail = digits.slice(-CARD_TAIL);
      const brand = read(entry, "brand");
      if (!tail) {
        return brand ?? displayName(entry.name);
      }
      return brand ? `${brand} •••• ${tail}` : `•••• ${tail}`;
    }
    case "identity": {
      const name = [read(entry, "given-name"), read(entry, "family-name")]
        .filter(Boolean)
        .join(" ");
      return name || read(entry, "email") || displayName(entry.name);
    }
    case "passkey":
      return read(entry, "rp") ?? displayName(entry.name);
    default:
      return read(entry, "username") ?? displayName(entry.name);
  }
}
