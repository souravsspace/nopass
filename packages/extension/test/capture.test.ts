import { beforeEach, describe, expect, it } from "vitest";
import {
  loginFor,
  readLogin,
  readSubmitted,
  submits,
  suggestedName,
} from "../lib/capture";
import { findLoginForms, type LoginForm } from "../lib/forms";

function render(html: string): LoginForm[] {
  document.body.innerHTML = html;
  return findLoginForms(document);
}

function first(forms: LoginForm[]): LoginForm {
  const [form] = forms;
  if (!form) {
    throw new Error("expected at least one login form");
  }
  return form;
}

describe("readLogin", () => {
  beforeEach(() => {
    document.body.innerHTML = "";
  });

  it("reads what was typed into the two fields", () => {
    const forms = render(`
      <form>
        <input type="text" value="sana" />
        <input type="password" value="hunter2" />
      </form>
    `);

    expect(readLogin(first(forms))).toEqual({
      password: "hunter2",
      username: "sana",
    });
  });

  it("offers nothing when the password box is empty", () => {
    const forms = render(
      `<form><input type="text" value="sana" /><input type="password" /></form>`
    );

    expect(readLogin(first(forms))).toBeNull();
  });

  it("leaves the username out rather than sending an empty one", () => {
    const forms = render(
      `<form><input type="text" value="  " /><input type="password" value="hunter2" /></form>`
    );

    expect(readLogin(first(forms))).toEqual({ password: "hunter2" });
  });

  it("has nothing to offer from a first step that has no password", () => {
    const forms = render(
      `<form><input type="email" name="identifier" value="sana@example.com" /></form>`
    );

    expect(readLogin(first(forms))).toBeNull();
  });
});

describe("submits", () => {
  it("recognises the controls a sign-in is sent with", () => {
    document.body.innerHTML = `
      <button id="b"><span id="inner">Sign in</span></button>
      <input id="s" type="submit" />
      <div id="r" role="button"></div>
      <p id="p">not a button</p>
    `;

    for (const id of ["b", "inner", "s", "r"]) {
      expect(submits(document.querySelector(`#${id}`))).toBe(true);
    }
    expect(submits(document.querySelector("#p"))).toBe(false);
    expect(submits(null)).toBe(false);
  });
});

describe("loginFor", () => {
  beforeEach(() => {
    document.body.innerHTML = "";
  });

  it("picks the form the click landed in", () => {
    const forms = render(`
      <form id="one">
        <input type="text" name="a" />
        <input type="password" value="one-pw" />
        <button id="go">Sign in</button>
      </form>
      <form id="two">
        <input type="text" name="b" />
        <input type="password" value="two-pw" />
      </form>
    `);

    const picked = loginFor(document.querySelector("#go"), forms);
    expect(picked?.password?.value).toBe("one-pw");
  });

  it("takes the only filled form when the click sits outside it", () => {
    const forms = render(`
      <form><input type="password" value="hunter2" /></form>
      <button id="go">Sign in</button>
    `);

    expect(
      loginFor(document.querySelector("#go"), forms)?.password?.value
    ).toBe("hunter2");
  });

  it("refuses to guess between two filled forms", () => {
    const forms = render(`
      <form><input type="password" value="one" /></form>
      <form><input type="password" value="two" /></form>
      <button id="go">Sign in</button>
    `);

    expect(loginFor(document.querySelector("#go"), forms)).toBeNull();
  });

  it("has nothing to offer when no password was typed", () => {
    const forms = render(`<form><input type="password" /></form>`);

    expect(loginFor(null, forms)).toBeNull();
  });
});

describe("suggestedName", () => {
  it("follows the convention the store already uses", () => {
    expect(suggestedName("github.com")).toBe("web/github.com");
  });
});

describe("readSubmitted", () => {
  beforeEach(() => {
    document.body.innerHTML = "";
  });

  it("reads a login when a password was typed", () => {
    const forms = render(`
      <form>
        <input type="text" value="sana" />
        <input type="password" value="hunter2" />
      </form>
    `);

    expect(readSubmitted(document, first(forms))).toEqual({
      fields: [{ key: "username", value: "sana" }],
      kind: "login",
      secret: "hunter2",
    });
  });

  it("reads a card off a checkout", () => {
    document.body.innerHTML = `
      <form>
        <input autocomplete="cc-name" value="Sana Qureshi" />
        <input autocomplete="cc-number" value="4111 1111 1111 4242" />
        <input autocomplete="cc-exp" value="04/29" />
        <input autocomplete="cc-csc" value="737" />
      </form>
    `;

    const captured = readSubmitted(document, null);
    expect(captured?.kind).toBe("card");
    // Spaces are how a card is printed, not how it is stored.
    expect(captured?.secret).toBe("4111111111114242");
    expect(captured?.fields).toContainEqual({
      key: "cardholder",
      value: "Sana Qureshi",
    });
    expect(captured?.fields).toContainEqual({ key: "exp-month", value: "04" });
    expect(captured?.fields).toContainEqual({ key: "exp-year", value: "2029" });
  });

  it("keeps a security code out of what it offers to save", () => {
    // Many issuers forbid storing one, and a prompt that quietly kept it
    // would be making that decision for the user.
    document.body.innerHTML = `
      <form>
        <input autocomplete="cc-number" value="4111111111114242" />
        <input autocomplete="cc-csc" value="737" />
      </form>
    `;

    const captured = readSubmitted(document, null);
    expect(captured?.fields.map((field) => field.key)).not.toContain("cvv");
  });

  it("reads an address when enough of one was typed", () => {
    document.body.innerHTML = `
      <form>
        <input autocomplete="given-name" value="Sana" />
        <input autocomplete="family-name" value="Qureshi" />
        <input autocomplete="address-line1" value="12 Example Road" />
        <input autocomplete="address-level2" value="Dhaka" />
        <input autocomplete="postal-code" value="1207" />
      </form>
    `;

    const captured = readSubmitted(document, null);
    expect(captured?.kind).toBe("identity");
    expect(captured?.secret).toBeUndefined();
    expect(captured?.fields).toContainEqual({ key: "city", value: "Dhaka" });
  });

  it("says nothing about a form with one address field filled in", () => {
    // A newsletter box asking for a name is not an address worth keeping.
    document.body.innerHTML = `
      <form><input autocomplete="given-name" value="Sana" /></form>
    `;

    expect(readSubmitted(document, null)).toBeNull();
  });

  it("prefers the login when a page has both", () => {
    const forms = render(`
      <form>
        <input type="text" value="sana" />
        <input type="password" value="hunter2" />
        <input autocomplete="cc-number" value="4111111111114242" />
      </form>
    `);

    expect(readSubmitted(document, first(forms))?.kind).toBe("login");
  });

  it("says nothing about an empty form", () => {
    document.body.innerHTML = `
      <form>
        <input autocomplete="cc-number" />
        <input autocomplete="given-name" />
      </form>
    `;

    expect(readSubmitted(document, null)).toBeNull();
  });
});
