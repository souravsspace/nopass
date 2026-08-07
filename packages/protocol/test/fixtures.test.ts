import { describe, expect, it } from "vitest";
import fixtures from "../fixtures/messages.json" with { type: "json" };
import { PROTOCOL_VERSION, requestSchema, responseSchema } from "../src/index";

describe("the fixture file", () => {
  it("agrees with the version the code announces", () => {
    expect(fixtures.protocolVersion).toBe(PROTOCOL_VERSION);
  });

  it("covers both accepted and rejected shapes on each side", () => {
    for (const side of [fixtures.requests, fixtures.responses]) {
      expect(side.some((c) => c.valid)).toBe(true);
      expect(side.some((c) => !c.valid)).toBe(true);
    }
  });
});

describe("requests", () => {
  for (const testCase of fixtures.requests.filter((c) => c.valid)) {
    it(`accepts: ${testCase.name}`, () => {
      expect(() => requestSchema.parse(testCase.json)).not.toThrow();
    });

    it(`round-trips: ${testCase.name}`, () => {
      const parsed = requestSchema.parse(testCase.json);
      expect(JSON.parse(JSON.stringify(parsed))).toEqual(testCase.json);
    });
  }

  for (const testCase of fixtures.requests.filter((c) => !c.valid)) {
    it(`rejects: ${testCase.name}`, () => {
      expect(requestSchema.safeParse(testCase.json).success).toBe(false);
    });
  }
});

describe("responses", () => {
  for (const testCase of fixtures.responses.filter((c) => c.valid)) {
    it(`accepts: ${testCase.name}`, () => {
      expect(() => responseSchema.parse(testCase.json)).not.toThrow();
    });

    it(`round-trips: ${testCase.name}`, () => {
      const parsed = responseSchema.parse(testCase.json);
      expect(JSON.parse(JSON.stringify(parsed))).toEqual(testCase.json);
    });
  }

  for (const testCase of fixtures.responses.filter((c) => !c.valid)) {
    it(`rejects: ${testCase.name}`, () => {
      expect(responseSchema.safeParse(testCase.json).success).toBe(false);
    });
  }
});

describe("the read-only boundary (ADR-0002)", () => {
  const mutating = ["insert", "edit", "rm", "mv", "cp", "delete", "remove"];

  for (const verb of mutating) {
    it(`has no schema for ${verb}`, () => {
      const result = requestSchema.safeParse({ entry: "a", id: 1, verb });
      expect(result.success).toBe(false);
    });
  }
});
