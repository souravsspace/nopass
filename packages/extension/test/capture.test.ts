import { beforeEach, describe, expect, it } from "vitest";
import { loginFor, readLogin, submits, suggestedName } from "../lib/capture";
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

    expect(loginFor(document.querySelector("#go"), forms)?.password?.value).toBe(
      "hunter2"
    );
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
