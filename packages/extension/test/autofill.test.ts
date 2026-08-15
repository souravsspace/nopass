import { beforeEach, describe, expect, it } from "vitest";
import {
  fieldKeyOf,
  fillFields,
  findFields,
  wantsWallet,
} from "../lib/autofill";

function render(html: string): Document {
  document.body.innerHTML = html;
  return document;
}

function input(selector: string): HTMLInputElement {
  const found = document.querySelector(selector);
  if (!(found instanceof HTMLInputElement)) {
    throw new Error(`expected an input at ${selector}`);
  }
  return found;
}

describe("fieldKeyOf", () => {
  beforeEach(() => {
    document.body.innerHTML = "";
  });

  it("reads the autocomplete token a well-written checkout gives it", () => {
    render(`
      <form>
        <input id="num" autocomplete="cc-number" />
        <input id="name" autocomplete="cc-name" />
        <input id="month" autocomplete="cc-exp-month" />
        <input id="year" autocomplete="cc-exp-year" />
        <input id="csc" autocomplete="cc-csc" />
      </form>
    `);

    expect(fieldKeyOf(input("#num"))).toBe("cc-number");
    expect(fieldKeyOf(input("#name"))).toBe("cardholder");
    expect(fieldKeyOf(input("#month"))).toBe("exp-month");
    expect(fieldKeyOf(input("#year"))).toBe("exp-year");
    expect(fieldKeyOf(input("#csc"))).toBe("cvv");
  });

  it("reads the address tokens the same way", () => {
    render(`
      <form>
        <input id="first" autocomplete="given-name" />
        <input id="last" autocomplete="family-name" />
        <input id="line1" autocomplete="address-line1" />
        <input id="city" autocomplete="address-level2" />
        <input id="region" autocomplete="address-level1" />
        <input id="post" autocomplete="postal-code" />
        <input id="country" autocomplete="country-name" />
        <input id="tel" autocomplete="tel" />
      </form>
    `);

    expect(fieldKeyOf(input("#first"))).toBe("given-name");
    expect(fieldKeyOf(input("#last"))).toBe("family-name");
    expect(fieldKeyOf(input("#line1"))).toBe("street");
    expect(fieldKeyOf(input("#city"))).toBe("city");
    expect(fieldKeyOf(input("#region"))).toBe("region");
    expect(fieldKeyOf(input("#post"))).toBe("postcode");
    expect(fieldKeyOf(input("#country"))).toBe("country");
    expect(fieldKeyOf(input("#tel"))).toBe("phone");
  });

  it("ignores the section and billing prefixes a token may carry", () => {
    render(`<input id="n" autocomplete="section-pay billing cc-number" />`);

    expect(fieldKeyOf(input("#n"))).toBe("cc-number");
  });

  it("falls back to a narrow reading of the name and id", () => {
    render(`
      <form>
        <input id="cardNumber" />
        <input id="x" name="cvv" />
        <input id="y" name="postcode" />
      </form>
    `);

    expect(fieldKeyOf(input("#cardNumber"))).toBe("cc-number");
    expect(fieldKeyOf(input("#x"))).toBe("cvv");
    expect(fieldKeyOf(input("#y"))).toBe("postcode");
  });

  it("leaves a field that says nothing alone", () => {
    render(`<input id="q" name="search" placeholder="Search" />`);

    expect(fieldKeyOf(input("#q"))).toBeNull();
  });

  it("does not mistake a password box for a card field", () => {
    render(`<input id="p" type="password" name="card-password" />`);

    expect(fieldKeyOf(input("#p"))).toBeNull();
  });
});

describe("wantsWallet", () => {
  beforeEach(() => {
    document.body.innerHTML = "";
  });

  it("is true for a card field", () => {
    render(`<input id="n" autocomplete="cc-number" />`);
    expect(wantsWallet(input("#n"))).toBe("card");
  });

  it("is true for an address field", () => {
    render(`<input id="c" autocomplete="address-level2" />`);
    expect(wantsWallet(input("#c"))).toBe("identity");
  });

  it("is nothing for a login field", () => {
    render(`<input id="u" autocomplete="username" />`);
    expect(wantsWallet(input("#u"))).toBeNull();
  });
});

describe("findFields", () => {
  beforeEach(() => {
    document.body.innerHTML = "";
  });

  it("collects every field of a checkout that it recognises", () => {
    render(`
      <form>
        <input id="num" autocomplete="cc-number" />
        <input id="csc" autocomplete="cc-csc" />
        <input id="q" name="search" />
      </form>
    `);

    const found = findFields(document);
    expect([...found.keys()].sort()).toEqual(["cc-number", "cvv"]);
  });

  it("takes the first of two fields that claim the same thing", () => {
    render(`
      <input id="a" autocomplete="cc-number" />
      <input id="b" autocomplete="cc-number" />
    `);

    expect(findFields(document).get("cc-number")?.id).toBe("a");
  });

  it("skips a field nobody can type into", () => {
    render(`<input id="n" autocomplete="cc-number" disabled />`);

    expect(findFields(document).size).toBe(0);
  });
});

describe("fillFields", () => {
  beforeEach(() => {
    document.body.innerHTML = "";
  });

  it("writes each value into the field that asked for it", () => {
    render(`
      <form>
        <input id="num" autocomplete="cc-number" />
        <input id="name" autocomplete="cc-name" />
        <input id="csc" autocomplete="cc-csc" />
      </form>
    `);

    const filled = fillFields(document, {
      cardholder: "Sana Qureshi",
      "cc-number": "4111111111111111",
      cvv: "737",
    });

    expect(filled).toBe(3);
    expect(input("#num").value).toBe("4111111111111111");
    expect(input("#name").value).toBe("Sana Qureshi");
    expect(input("#csc").value).toBe("737");
  });

  it("announces each change so a framework notices it", () => {
    render(`<input id="num" autocomplete="cc-number" />`);
    const seen: string[] = [];
    for (const type of ["input", "change"]) {
      input("#num").addEventListener(type, (event) => seen.push(event.type));
    }

    fillFields(document, { "cc-number": "4111111111111111" });

    expect(seen).toEqual(["input", "change"]);
  });

  it("leaves a field alone when the entry has nothing for it", () => {
    render(`
      <input id="num" autocomplete="cc-number" />
      <input id="csc" autocomplete="cc-csc" value="typed" />
    `);

    fillFields(document, { "cc-number": "4111111111111111" });

    expect(input("#csc").value).toBe("typed");
  });

  it("fills a two-digit expiry into a field that wants the whole thing", () => {
    render(`<input id="exp" autocomplete="cc-exp" />`);

    fillFields(document, { "exp-month": "04", "exp-year": "2029" });

    expect(input("#exp").value).toBe("04/29");
  });

  it("shortens the year for a field that only holds two digits", () => {
    render(`<input id="year" autocomplete="cc-exp-year" maxlength="2" />`);

    fillFields(document, { "exp-year": "2029" });

    expect(input("#year").value).toBe("29");
  });
});
