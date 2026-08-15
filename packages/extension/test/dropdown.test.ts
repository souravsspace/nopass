import { beforeEach, describe, expect, it, vi } from "vitest";
import { displayName, folderOf, renderDropdown } from "../lib/dropdown";

const LOCKED = /locked/i;
const INIT = /nopass init/;
const UNREACHABLE = /not reachable/i;

function shadow(): ShadowRoot {
  document.body.innerHTML = "<div id='host'></div>";
  const host = document.querySelector("#host") as HTMLElement;
  return host.attachShadow({ mode: "open" });
}

describe("entry names", () => {
  it("shows the leaf, files the rest", () => {
    expect(displayName("web/personal/github.com")).toBe("github.com");
    expect(folderOf("web/personal/github.com")).toBe("web/personal");
  });

  it("copes with an entry that has no folder", () => {
    expect(displayName("github.com")).toBe("github.com");
    expect(folderOf("github.com")).toBeNull();
  });
});

describe("renderDropdown", () => {
  let root: ShadowRoot;

  beforeEach(() => {
    root = shadow();
  });

  it("renders one row per match", () => {
    renderDropdown(root, {
      kind: "matches",
      matches: [
        { kind: "login", name: "web/a.com" },
        { kind: "login", name: "web/b.com" },
      ],
      onPick: () => undefined,
    });

    expect(root.querySelectorAll(".np-row")).toHaveLength(2);
  });

  it("leads with the username when there is one", () => {
    renderDropdown(root, {
      kind: "matches",
      matches: [
        { kind: "login", name: "web/a.com", username: "sana@example.com" },
      ],
      onPick: () => undefined,
    });

    expect(root.querySelector(".np-name")?.textContent).toBe(
      "sana@example.com"
    );
    expect(root.querySelector(".np-meta")?.textContent).toBe("a.com");
  });

  it("picks on mousedown, because focusout would beat a click to it", () => {
    const onPick = vi.fn();
    renderDropdown(root, {
      kind: "matches",
      matches: [{ kind: "login", name: "web/a.com" }],
      onPick,
    });

    const row = root.querySelector(".np-row") as HTMLElement;
    row.dispatchEvent(
      new MouseEvent("mousedown", { bubbles: true, cancelable: true })
    );

    expect(onPick).toHaveBeenCalledWith("web/a.com");
  });

  it("never puts a secret in the DOM", () => {
    renderDropdown(root, {
      kind: "matches",
      matches: [
        {
          kind: "login",
          name: "web/a.com",
          url: "https://a.com",
          username: "sana",
        },
      ],
      onPick: () => undefined,
    });

    expect(root.innerHTML).not.toContain("password");
  });

  it("replaces what was there rather than stacking panels", () => {
    for (const name of ["web/a.com", "web/b.com"]) {
      renderDropdown(root, {
        kind: "matches",
        matches: [{ kind: "login", name }],
        onPick: () => undefined,
      });
    }

    expect(root.querySelectorAll(".np")).toHaveLength(1);
    expect(root.querySelectorAll(".np-row")).toHaveLength(1);
  });

  it("explains itself instead of showing an empty list when locked", () => {
    renderDropdown(root, { kind: "locked", state: { status: "locked" } });
    expect(root.querySelector(".np-empty")?.textContent).toMatch(LOCKED);

    renderDropdown(root, { kind: "locked", state: { status: "no-store" } });
    expect(root.querySelector(".np-empty")?.textContent).toMatch(INIT);

    renderDropdown(root, {
      kind: "locked",
      state: { message: "x", status: "unavailable" },
    });
    expect(root.querySelector(".np-empty")?.textContent).toMatch(UNREACHABLE);
  });

  it("announces itself to assistive technology", () => {
    renderDropdown(root, {
      kind: "matches",
      matches: [{ kind: "login", name: "web/a.com" }],
      onPick: () => undefined,
    });

    expect(root.querySelector(".np")?.getAttribute("role")).toBe("listbox");
    expect(root.querySelector(".np-row")?.getAttribute("role")).toBe("option");
  });
});
