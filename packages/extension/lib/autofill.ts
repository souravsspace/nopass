/**
 * Cards and addresses, into the fields of a page.
 *
 * A login form is two boxes with a shape worth recognising. A checkout is
 * twenty, and guessing wrong there does not mean a failed sign-in — it means
 * a card number in the wrong box, on somebody else's page. So the reading is
 * deliberately literal:
 *
 * 1. `autocomplete`, which is the standard answer and the only one a page can
 *    be held to;
 * 2. a narrow reading of `name` and `id` for the fields that are unambiguous
 *    in every checkout anyone writes — `cardNumber`, `cvv`, `postcode`.
 *
 * Nothing here fills on its own. A card reaches a page because the user
 * picked it, every time.
 */

/** The wire's key for a card's number, which is a record's first line. */
export const CARD_NUMBER = "cc-number";

/** `autocomplete` token → the canonical key the wire uses. */
const TOKENS: Record<string, string> = {
  "additional-name": "given-name",
  "address-level1": "region",
  "address-level2": "city",
  "address-line1": "street",
  "address-line2": "street2",
  "cc-csc": "cvv",
  "cc-exp": "exp",
  "cc-exp-month": "exp-month",
  "cc-exp-year": "exp-year",
  "cc-name": "cardholder",
  "cc-number": CARD_NUMBER,
  "cc-type": "brand",
  country: "country",
  "country-name": "country",
  email: "email",
  "family-name": "family-name",
  "given-name": "given-name",
  organization: "organization",
  "postal-code": "postcode",
  "street-address": "street",
  tel: "phone",
  "tel-national": "phone",
};

/**
 * The fallback, applied to `name` and `id` only.
 *
 * Every pattern here has to be one nobody writes for something else: `cvv`
 * means a security code on every checkout there is, while `number` on its own
 * could be anything, so it is not here.
 */
const PATTERNS: [RegExp, string][] = [
  [/(card|cc)[-_]?(number|num|no)|creditcard/i, CARD_NUMBER],
  [/(cvv|cvc|csc|securitycode|security[-_]code)/i, "cvv"],
  [/(card|cc)[-_]?(holder|name)|nameoncard/i, "cardholder"],
  [/(exp)[-_]?(month|mm)$|^month$/i, "exp-month"],
  [/(exp)[-_]?(year|yy)$|^year$/i, "exp-year"],
  [/(expiry|expiration|exp[-_]?date)/i, "exp"],
  [/(post(al)?[-_]?code|zip[-_]?code|^zip$)/i, "postcode"],
  [/(first[-_]?name|given[-_]?name|forename)/i, "given-name"],
  [/(last[-_]?name|family[-_]?name|surname)/i, "family-name"],
  [/(street|address[-_]?1|addressline1)/i, "street"],
  [/(address[-_]?2|addressline2|apartment|^unit$)/i, "street2"],
  [/(^city$|town|locality)/i, "city"],
  [/(^state$|province|region|county)/i, "region"],
  [/(^country$|countryname)/i, "country"],
  [/(^phone|telephone|^tel$|mobile)/i, "phone"],
  [/(^company$|organisation|organization|employer)/i, "organization"],
];

/** Card keys, so a focused field can say which kind it is asking for. */
const CARD_KEYS = new Set([
  CARD_NUMBER,
  "brand",
  "cardholder",
  "cvv",
  "exp",
  "exp-month",
  "exp-year",
]);

/** `autocomplete` is a space-separated list, most specific token last. */
const SPACES = /\s+/;

/** How a card prints its expiry when a page asks for the whole thing. */
const SHORT_YEAR = 2;

function fillable(input: HTMLInputElement): boolean {
  return !(
    input.disabled ||
    input.readOnly ||
    input.hidden ||
    input.type === "hidden" ||
    // A card never goes in a password box, whatever the box is called.
    input.type === "password"
  );
}

/** The canonical key this field is asking for, if it says so. */
export function fieldKeyOf(input: HTMLInputElement): string | null {
  if (!fillable(input)) {
    return null;
  }

  // `autocomplete` may carry a section and a billing/shipping hint before the
  // token that matters: `section-pay billing cc-number`.
  for (const token of input.autocomplete
    .toLowerCase()
    .split(SPACES)
    .reverse()) {
    const key = TOKENS[token];
    if (key) {
      return key;
    }
  }

  const described = `${input.name} ${input.id}`;
  for (const [pattern, key] of PATTERNS) {
    if (pattern.test(described)) {
      return key;
    }
  }
  return null;
}

/** Which kind of entry a focused field is asking for, if either. */
export function wantsWallet(
  input: HTMLInputElement
): "card" | "identity" | null {
  const key = fieldKeyOf(input);
  if (!key) {
    return null;
  }
  return CARD_KEYS.has(key) ? "card" : "identity";
}

/** Every field under `root` that names itself, first occurrence winning. */
export function findFields(root: ParentNode): Map<string, HTMLInputElement> {
  const found = new Map<string, HTMLInputElement>();
  for (const input of root.querySelectorAll("input")) {
    const key = fieldKeyOf(input);
    if (key && !found.has(key)) {
      found.set(key, input);
    }
  }
  return found;
}

/**
 * Write `values` into whichever fields asked for them.
 *
 * Returns how many landed, so a caller can tell "nothing here wanted this"
 * from "done". A field the entry has nothing for is left exactly as the user
 * left it.
 */
export function fillFields(
  root: ParentNode,
  values: Record<string, string | undefined>
): number {
  const fields = findFields(root);
  let filled = 0;

  for (const [key, input] of fields) {
    const value = valueFor(key, values, input);
    if (value === undefined || value === "") {
      continue;
    }
    setValue(input, value);
    filled += 1;
  }
  return filled;
}

/** What this field should hold, given what the entry carries. */
function valueFor(
  key: string,
  values: Record<string, string | undefined>,
  input: HTMLInputElement
): string | undefined {
  // A page that wants the expiry in one box wants it the way the card prints
  // it, and one that wants the year alone may only have room for two digits.
  if (key === "exp") {
    const month = values["exp-month"];
    const year = values["exp-year"];
    return month && year ? `${month}/${year.slice(-SHORT_YEAR)}` : undefined;
  }

  const value = values[key];
  if (value && key === "exp-year" && input.maxLength === SHORT_YEAR) {
    return value.slice(-SHORT_YEAR);
  }
  return value;
}

/**
 * Write a value the way a keystroke would.
 *
 * The same reasoning as filling a login: React and Vue track the value on the
 * element and treat a plain assignment as something they did themselves, so
 * the field would look filled to the user and empty to the page.
 */
function setValue(input: HTMLInputElement, value: string): void {
  const setter = Object.getOwnPropertyDescriptor(
    HTMLInputElement.prototype,
    "value"
  )?.set;
  if (setter) {
    setter.call(input, value);
  } else {
    input.value = value;
  }

  input.dispatchEvent(new Event("input", { bubbles: true }));
  input.dispatchEvent(new Event("change", { bubbles: true }));
}
