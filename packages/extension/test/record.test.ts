import type { Secret } from "@nopass/protocol";
import { describe, expect, it } from "vitest";
import {
  fieldsFor,
  labelFor,
  read,
  title,
  toFields,
  withField,
} from "../lib/record";

function secret(over: Partial<Secret> = {}): Secret {
  return {
    fields: [
      { key: "username", value: "sana@example.com" },
      { key: "url", value: "https://a.b" },
    ],
    kind: "login",
    name: "web/a.b",
    secret: "hunter2",
    ...over,
  };
}

describe("read", () => {
  it("finds a field by its canonical key", () => {
    expect(read(secret(), "username")).toBe("sana@example.com");
  });

  it("is undefined for a field the entry does not carry", () => {
    expect(read(secret(), "totp")).toBeUndefined();
  });
});

describe("withField", () => {
  it("replaces a value in place, leaving the order alone", () => {
    const next = withField(secret().fields, "username", "someone@else");

    expect(next).toEqual([
      { key: "username", value: "someone@else" },
      { key: "url", value: "https://a.b" },
    ]);
  });

  it("appends a field the entry did not have", () => {
    const next = withField(secret().fields, "totp", "ABC");

    expect(next.at(-1)).toEqual({ key: "totp", value: "ABC" });
  });

  it("keeps an empty value, because that is how a field is cleared", () => {
    // The host reads an empty value as "remove this field", so it has to
    // survive the trip rather than being dropped as nothing to say.
    expect(withField(secret().fields, "username", "")).toContainEqual({
      key: "username",
      value: "",
    });
  });
});

describe("fieldsFor", () => {
  it("names what a login screen asks for", () => {
    expect(fieldsFor("login")).toEqual(["username", "url", "totp"]);
  });

  it("names what a card asks for, in the order a card is read", () => {
    expect(fieldsFor("card")).toEqual([
      "cardholder",
      "exp-month",
      "exp-year",
      "cvv",
      "brand",
      "zip",
    ]);
  });

  it("covers the parts of an address a checkout asks for", () => {
    const identity = fieldsFor("identity");

    for (const key of [
      "given-name",
      "family-name",
      "street",
      "city",
      "country",
    ]) {
      expect(identity).toContain(key);
    }
  });
});

describe("labelFor", () => {
  it("writes a key the way a person would read it", () => {
    expect(labelFor("username")).toBe("Username");
    expect(labelFor("exp-month")).toBe("Expiry month");
    expect(labelFor("cvv")).toBe("Security code");
    expect(labelFor("given-name")).toBe("First name");
  });

  it("falls back to the key itself for a field it has no name for", () => {
    expect(labelFor("favourite-colour")).toBe("Favourite colour");
  });
});

describe("title", () => {
  it("leads with the username for a login", () => {
    expect(title(secret())).toBe("sana@example.com");
  });

  it("leads with the person for an identity", () => {
    const identity = secret({
      fields: [
        { key: "given-name", value: "Sana" },
        { key: "family-name", value: "Qureshi" },
      ],
      kind: "identity",
      name: "me/home",
      secret: "",
    });

    expect(title(identity)).toBe("Sana Qureshi");
  });

  it("never puts a card number on screen, only its tail", () => {
    const card = secret({
      fields: [{ key: "brand", value: "Visa" }],
      kind: "card",
      name: "cards/visa",
      secret: "4111111111114242",
    });

    expect(title(card)).toBe("Visa •••• 4242");
  });

  it("falls back to the entry's own name", () => {
    expect(title(secret({ fields: [], name: "web/personal/a.b" }))).toBe("a.b");
  });
});

describe("toFields", () => {
  it("drops the fields nobody filled in", () => {
    const fields = toFields({ totp: undefined, url: "", username: "sana" });

    expect(fields).toEqual([{ key: "username", value: "sana" }]);
  });

  it("keeps the order the caller listed", () => {
    // Built from pairs rather than a literal: the screen hands these over in
    // the order it renders them, which is the order they should be written.
    const fields = toFields(
      Object.fromEntries([
        ["given-name", "Sana"],
        ["family-name", "Qureshi"],
      ])
    );

    expect(fields.map((field) => field.key)).toEqual([
      "given-name",
      "family-name",
    ]);
  });
});
