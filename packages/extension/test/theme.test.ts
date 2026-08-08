import { beforeEach, describe, expect, it } from "vitest";
import { followSystemTheme } from "../lib/theme";

/**
 * A `matchMedia` that can change its mind, the way a system flipping to dark
 * at sunset does. happy-dom answers every query with `false` and never fires,
 * so the popup's reaction to a change has to be driven by hand.
 */
function stubSystem(dark: boolean) {
  const listeners = new Set<() => void>();
  const query = {
    addEventListener: (_: string, listener: () => void) => {
      listeners.add(listener);
    },
    matches: dark,
    removeEventListener: (_: string, listener: () => void) => {
      listeners.delete(listener);
    },
  };
  window.matchMedia = (() => query) as unknown as typeof window.matchMedia;
  return (next: boolean) => {
    query.matches = next;
    for (const listener of listeners) {
      listener();
    }
  };
}

const isDark = () => document.documentElement.classList.contains("dark");

describe("followSystemTheme", () => {
  beforeEach(() => {
    document.documentElement.classList.remove("dark");
  });

  it("wears the system's colours from the first paint", () => {
    stubSystem(true);
    followSystemTheme();
    expect(isDark()).toBe(true);
  });

  it("leaves a light system light", () => {
    stubSystem(false);
    followSystemTheme();
    expect(isDark()).toBe(false);
  });

  it("follows a system that changes while the popup is open", () => {
    const set = stubSystem(false);
    followSystemTheme();

    set(true);
    expect(isDark()).toBe(true);
    set(false);
    expect(isDark()).toBe(false);
  });
});
