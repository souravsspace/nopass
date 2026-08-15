import { beforeEach, describe, expect, it, vi } from "vitest";
import type { LoginForm } from "../lib/forms";
import { fillLogin, findLoginForms } from "../lib/forms";

/** The first result, or a failure that says so rather than a type error. */
function first(forms: LoginForm[]): LoginForm {
  const [form] = forms;
  if (!form) {
    throw new Error("expected at least one login form");
  }
  return form;
}

/** The password field, or a failure that says so rather than a type error. */
function passwordOf(form: LoginForm): HTMLInputElement {
  if (!form.password) {
    throw new Error("expected a password field");
  }
  return form.password;
}

function render(html: string): Document {
  document.body.innerHTML = html;
  return document;
}

describe("findLoginForms", () => {
  beforeEach(() => {
    document.body.innerHTML = "";
  });

  it("finds a password field and the username above it", () => {
    render(`
      <form>
        <input type="text" name="user" />
        <input type="password" name="pass" />
      </form>
    `);

    const forms = findLoginForms(document);
    const form = first(forms);
    const rest = forms.slice(1);
    expect(rest).toHaveLength(0);
    expect(passwordOf(form).getAttribute("name")).toBe("pass");
    expect(form.username?.getAttribute("name")).toBe("user");
  });

  it("treats an email field as the username", () => {
    render(`<form><input type="email" /><input type="password" /></form>`);
    expect(first(findLoginForms(document)).username?.getAttribute("type")).toBe(
      "email"
    );
  });

  it("finds a login form that is not wrapped in a form element", () => {
    render(
      `<div><input type="text" id="u" /><input type="password" id="p" /></div>`
    );

    const form = first(findLoginForms(document));
    expect(form.username?.id).toBe("u");
    expect(form.form).toBeNull();
  });

  it("copes with a password field that has no username beside it", () => {
    render(`<form><input type="password" /></form>`);

    const form = first(findLoginForms(document));
    expect(form.username).toBeNull();
  });

  it("ignores a form with more than one password field", () => {
    // Two password boxes means signing up or changing a password, not
    // signing in. Filling the existing credential there is wrong at best.
    render(`
      <form>
        <input type="text" />
        <input type="password" name="new" />
        <input type="password" name="confirm" />
      </form>
    `);

    expect(findLoginForms(document)).toHaveLength(0);
  });

  it("ignores hidden and disabled fields", () => {
    render(`
      <form>
        <input type="text" />
        <input type="password" hidden />
      </form>
      <form>
        <input type="text" />
        <input type="password" disabled />
      </form>
      <form>
        <input type="text" />
        <input type="password" readonly />
      </form>
    `);

    expect(findLoginForms(document)).toHaveLength(0);
  });

  it("does not take a field that follows the password as the username", () => {
    render(`
      <form>
        <input type="password" />
        <input type="text" name="captcha" />
      </form>
    `);

    expect(first(findLoginForms(document)).username).toBeNull();
  });

  it("finds each login form on a page that has several", () => {
    render(`
      <form><input type="text" name="a" /><input type="password" /></form>
      <form><input type="text" name="b" /><input type="password" /></form>
    `);

    const names = findLoginForms(document).map((f: LoginForm) =>
      f.username?.getAttribute("name")
    );
    expect(names).toEqual(["a", "b"]);
  });

  it("skips a search box that happens to sit before a password", () => {
    render(`
      <form>
        <input type="search" name="q" />
        <input type="password" />
      </form>
    `);

    expect(first(findLoginForms(document)).username).toBeNull();
  });

  it("finds the account field of a sign-in that asks for it first", () => {
    render(`<form><input type="email" name="identifier" /></form>`);

    const form = first(findLoginForms(document));
    expect(form.username?.getAttribute("name")).toBe("identifier");
    expect(form.password).toBeNull();
  });

  it("takes a text field that says it holds a username", () => {
    render(
      `<form><input autocomplete="username" name="whatever" /></form>`
    );

    expect(first(findLoginForms(document)).username?.getAttribute("name")).toBe(
      "whatever"
    );
  });

  it("leaves a text field that claims nothing alone", () => {
    render(`<form><input type="text" name="postcode" /></form>`);

    expect(findLoginForms(document)).toHaveLength(0);
  });

  it("does not reopen a sign-up form through its email field", () => {
    // The two password boxes ruled this form out; the address above them is
    // not a second chance to fill it.
    render(`
      <form>
        <input type="email" name="email" />
        <input type="password" name="new" />
        <input type="password" name="confirm" />
      </form>
    `);

    expect(findLoginForms(document)).toHaveLength(0);
  });
});

describe("fillLogin", () => {
  beforeEach(() => {
    document.body.innerHTML = "";
  });

  it("puts the values where they belong", () => {
    render(`<form><input type="text" /><input type="password" /></form>`);
    const form = first(findLoginForms(document));

    fillLogin(form, { password: "hunter2", username: "sana" });

    expect(form.username?.value).toBe("sana");
    expect(passwordOf(form).value).toBe("hunter2");
  });

  it("announces the change so a framework notices it", () => {
    // React tracks the value on the DOM node and ignores a plain assignment.
    // Without the native setter and the events, the field looks filled to the
    // user and empty to the page.
    render(`<form><input type="password" /></form>`);
    const form = first(findLoginForms(document));
    const seen: string[] = [];
    for (const type of ["input", "change"]) {
      passwordOf(form).addEventListener(type, (event) => {
        seen.push(event.type);
        expect(event.bubbles).toBe(true);
      });
    }

    fillLogin(form, { password: "hunter2" });

    expect(seen).toEqual(["input", "change"]);
  });

  it("leaves the username alone when the entry has none", () => {
    render(
      `<form><input type="text" value="typed" /><input type="password" /></form>`
    );
    const form = first(findLoginForms(document));

    fillLogin(form, { password: "hunter2" });

    expect(form.username?.value).toBe("typed");
  });

  it("fills the account name alone on a first step that has no password", () => {
    render(`<form><input type="email" name="identifier" /></form>`);
    const form = first(findLoginForms(document));

    fillLogin(form, { password: "hunter2", username: "sana@example.com" });

    expect(form.username?.value).toBe("sana@example.com");
  });

  it("focuses the password so the user can see it landed", () => {
    render(`<form><input type="text" /><input type="password" /></form>`);
    const form = first(findLoginForms(document));
    const focus = vi.spyOn(passwordOf(form), "focus");

    fillLogin(form, { password: "hunter2" });

    expect(focus).toHaveBeenCalled();
  });
});
