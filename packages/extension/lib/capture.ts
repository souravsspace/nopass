/**
 * Noticing that a login was just submitted.
 *
 * This is the one place that reads a value back out of a page, and only ever
 * the two fields the user typed into a login form of their own accord. What is
 * read never reaches the store on its own: it becomes an offer the user
 * accepts by name in the prompt (ADR-0007).
 */

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
