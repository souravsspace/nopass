/**
 * Finding login forms on a page, and filling them.
 *
 * This is the only part of the extension that touches a page's DOM, and the
 * page is hostile by assumption. Nothing here reads a value out of the page or
 * sends anything back; it locates fields and writes into them.
 */

export interface LoginForm {
  /** The enclosing `<form>`, when the page bothered with one. */
  form: HTMLFormElement | null;
  /** The password field. There is always exactly one per login form. */
  password: HTMLInputElement;
  /** The field the username goes in, if the page has one. */
  username: HTMLInputElement | null;
}

/** Field types that can plausibly hold an account name. `search` is not one. */
const USERNAME_TYPES = new Set(["text", "email", "tel", ""]);

function isFillable(input: HTMLInputElement): boolean {
  return !(
    input.disabled ||
    input.readOnly ||
    input.hidden ||
    input.type === "hidden"
  );
}

/**
 * The username belongs to the nearest fillable text field *before* the
 * password within the same container. Anything after it is a second factor, a
 * captcha, or the next form entirely.
 */
function usernameFor(
  password: HTMLInputElement,
  scope: ParentNode
): HTMLInputElement | null {
  const inputs = [...scope.querySelectorAll("input")];
  const passwordIndex = inputs.indexOf(password);

  for (let index = passwordIndex - 1; index >= 0; index--) {
    const candidate = inputs[index];
    if (
      candidate &&
      isFillable(candidate) &&
      USERNAME_TYPES.has(candidate.type)
    ) {
      return candidate;
    }
  }
  return null;
}

/**
 * Every login form under `root`.
 *
 * A container holding two or more password fields is a sign-up or a
 * change-password flow, and the stored credential does not belong in either,
 * so it is skipped rather than guessed at.
 */
export function findLoginForms(root: ParentNode): LoginForm[] {
  const passwords = [
    ...root.querySelectorAll<HTMLInputElement>("input[type='password']"),
  ].filter(isFillable);

  const forms: LoginForm[] = [];
  for (const password of passwords) {
    const form = password.closest("form");
    const scope: ParentNode = form ?? password.parentElement ?? root;

    const siblings = [
      ...scope.querySelectorAll("input[type='password']"),
    ].filter((input) => isFillable(input as HTMLInputElement));
    if (siblings.length > 1) {
      continue;
    }

    forms.push({ form, password, username: usernameFor(password, scope) });
  }
  return forms;
}

/**
 * Write a value the way a keystroke would.
 *
 * React and Vue track the value on the element and treat a plain assignment as
 * something they did themselves, so the field would look filled to the user
 * and empty to the page. Going through the native setter and then announcing
 * the change is what makes the two agree.
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

/** Fill `form` with a secret. A missing username leaves that field untouched. */
export function fillLogin(
  form: LoginForm,
  secret: { password: string; username?: string | undefined }
): void {
  if (form.username && secret.username) {
    setValue(form.username, secret.username);
  }
  setValue(form.password, secret.password);

  // Leaving the caret in the password field is the confirmation that
  // something happened — the page may well have restyled the box.
  form.password.focus();
}
