/**
 * Noticing that a login was just submitted.
 *
 * This is the one place that reads a value back out of a page, and only ever
 * the two fields the user typed into a login form of their own accord. What is
 * read never reaches the store on its own: it becomes an offer the user
 * accepts by name in the prompt (ADR-0007).
 */

import type { Field, Kind } from "@nopass/protocol";
import { CARD_NUMBER, findFields } from "./autofill";
import type { LoginForm } from "./forms";

/** A login the page was just given, on its way to being offered for saving. */
export interface Captured {
  password: string;
  username?: string;
}

/** What the user typed into `form`, if there is a password there to save. */
export function readLogin(form: LoginForm): Captured | null {
  const password = form.password?.value ?? "";
  if (!password) {
    return null;
  }

  const username = form.username?.value.trim() ?? "";
  return username ? { password, username } : { password };
}

/**
 * Whether `target` is the sort of control that submits a login.
 *
 * A form-less sign-in has no `submit` event to listen for, so the click on its
 * button is the only signal there is. The click may land on a span inside the
 * button, hence the walk upwards.
 */
export function submits(target: EventTarget | null): boolean {
  if (!(target instanceof Element)) {
    return false;
  }
  return (
    target.closest(
      "button, input[type='submit'], input[type='button'], [role='button']"
    ) !== null
  );
}

/**
 * The login `target` belongs to, out of the ones with something in them.
 *
 * A page with two filled login forms and a click that sits inside neither is
 * ambiguous, and guessing there would mean offering to save a password the
 * user did not just send anywhere.
 */
export function loginFor(
  target: EventTarget | null,
  forms: LoginForm[]
): LoginForm | null {
  const filled = forms.filter((form) => form.password?.value);
  if (filled.length === 0) {
    return null;
  }

  if (target instanceof Element) {
    const enclosing = target.closest("form");
    const owning = filled.find(
      (form) => form.form !== null && form.form === enclosing
    );
    if (owning) {
      return owning;
    }
  }

  return filled.length === 1 ? (filled[0] ?? null) : null;
}

/** The name a captured login is offered under, following `web/<host>`. */
export function suggestedName(host: string): string {
  return `web/${host}`;
}

/** What was just submitted, in the shape the store keeps it. */
export interface CapturedRecord {
  fields: Field[];
  kind: Kind;
  /** The first line: a password, a card number, absent for an address. */
  secret?: string;
}

/** Card fields worth keeping. `cvv` is deliberately absent — see below. */
const CARD_FIELDS = ["cardholder", "brand", "zip"];

/** Address fields, and how many have to be there before this is an address. */
const IDENTITY_FIELDS = [
  "given-name",
  "family-name",
  "email",
  "phone",
  "street",
  "street2",
  "city",
  "region",
  "postcode",
  "country",
  "organization",
];
const ENOUGH_OF_AN_ADDRESS = 3;

/** How a card number is printed, and how an expiry is written. */
const SPACES_AND_DASHES = /[\s-]/g;
const EXPIRY_SEPARATOR = /[/-]/;

/**
 * What the page was just given, if it is worth offering to keep.
 *
 * A login wins when there is one: a checkout that also asks you to sign in is
 * offering the login, and the card can be offered on the next submit. Below
 * that, a card number beats an address, because a page with both is a
 * checkout and the card is the part nobody wants to type again.
 */
export function readSubmitted(
  root: ParentNode,
  form: LoginForm | null
): CapturedRecord | null {
  const login = form ? readLogin(form) : null;
  if (login) {
    return {
      fields: login.username
        ? [{ key: "username", value: login.username }]
        : [],
      kind: "login",
      secret: login.password,
    };
  }

  const filled = new Map<string, string>();
  for (const [key, input] of findFields(root)) {
    const value = input.value.trim();
    if (value) {
      filled.set(key, value);
    }
  }

  return readCard(filled) ?? readIdentity(filled);
}

function readCard(filled: Map<string, string>): CapturedRecord | null {
  const number = filled.get(CARD_NUMBER)?.replace(SPACES_AND_DASHES, "");
  if (!number) {
    return null;
  }

  const fields: Field[] = [];
  for (const key of CARD_FIELDS) {
    const value = filled.get(key);
    if (value) {
      fields.push({ key, value });
    }
  }

  const [month, year] = expiry(filled);
  if (month && year) {
    fields.push({ key: "exp-month", value: month });
    fields.push({ key: "exp-year", value: year });
  }

  /*
   * The security code is read off the page and then left behind on purpose.
   * Card networks forbid storing it, and a prompt that kept it because it
   * happened to be on screen would be deciding that for the user. Someone who
   * wants it stored can add it on the entry screen.
   */
  return { fields, kind: "card", secret: number };
}

/** `04/29` in one box, or a month and a year in two. Always four-digit year. */
function expiry(filled: Map<string, string>): [string?, string?] {
  const combined = filled.get("exp");
  const parts = combined?.split(EXPIRY_SEPARATOR);
  const month = filled.get("exp-month") ?? parts?.[0]?.trim();
  const year = filled.get("exp-year") ?? parts?.[1]?.trim();
  if (!(month && year)) {
    return [];
  }
  const padded = month.padStart(2, "0");
  return [padded, year.length === 2 ? `20${year}` : year];
}

function readIdentity(filled: Map<string, string>): CapturedRecord | null {
  const fields: Field[] = [];
  for (const key of IDENTITY_FIELDS) {
    const value = filled.get(key);
    if (value) {
      fields.push({ key, value });
    }
  }

  // One name in a newsletter box is not an address. Three parts of one is.
  return fields.length >= ENOUGH_OF_AN_ADDRESS
    ? { fields, kind: "identity" }
    : null;
}
