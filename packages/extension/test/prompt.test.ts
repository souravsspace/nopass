import { beforeEach, describe, expect, it, vi } from "vitest";
import { renderSavePrompt, showPromptError } from "../lib/prompt";

function shadow(): ShadowRoot {
  document.body.innerHTML = "<div id='host'></div>";
  const host = document.querySelector("#host") as HTMLElement;
  return host.attachShadow({ mode: "open" });
}

function view(
  root: ShadowRoot,
  over: Partial<Parameters<typeof renderSavePrompt>[1]> = {}
) {
  renderSavePrompt(root, {
    detail: "sana@example.com",
    host: "github.com",
    kind: "login",
    onDismiss: () => undefined,
    onSave: () => undefined,
    suggestion: "web/github.com",
    ...over,
  });
}

function nameField(root: ShadowRoot): HTMLInputElement {
  const field = root.querySelector(".np-save-input");
  if (!(field instanceof HTMLInputElement)) {
    throw new Error("expected a name field");
  }
  return field;
}

function button(root: ShadowRoot, label: string): HTMLButtonElement {
  const found = [...root.querySelectorAll("button")].find(
    (candidate) => candidate.textContent === label
  );
  if (!found) {
    throw new Error(`expected a ${label} button`);
  }
  return found;
}

describe("renderSavePrompt", () => {
  let root: ShadowRoot;

  beforeEach(() => {
    root = shadow();
  });

  it("says which login it is offering to save", () => {
    view(root);

    expect(root.querySelector(".np-save-meta")?.textContent).toBe(
      "sana@example.com — github.com"
    );
    expect(nameField(root).value).toBe("web/github.com");
  });

  it("falls back to the host when the page had no username field", () => {
    view(root, { detail: undefined });

    expect(root.querySelector(".np-save-meta")?.textContent).toBe("github.com");
  });

  it("saves under whatever name the user leaves in the box", () => {
    const onSave = vi.fn();
    view(root, { onSave });

    nameField(root).value = "work/github";
    button(root, "Save").click();

    expect(onSave).toHaveBeenCalledWith("work/github");
  });

  it("does not save an empty name", () => {
    const onSave = vi.fn();
    view(root, { onSave });

    nameField(root).value = "   ";
    button(root, "Save").click();

    expect(onSave).not.toHaveBeenCalled();
  });

  it("dismisses without saving", () => {
    const onSave = vi.fn();
    const onDismiss = vi.fn();
    view(root, { onDismiss, onSave });

    button(root, "Not now").click();

    expect(onDismiss).toHaveBeenCalled();
    expect(onSave).not.toHaveBeenCalled();
  });

  it("asks the question that suits what was submitted", () => {
    view(root, { detail: "•••• 4242", kind: "card" });
    expect(root.querySelector(".np-save-title")?.textContent).toBe(
      "Save this card?"
    );

    view(root, { detail: "Sana Qureshi", kind: "identity" });
    expect(root.querySelector(".np-save-title")?.textContent).toBe(
      "Save these details?"
    );
  });

  it("replaces the panel rather than stacking a second one", () => {
    view(root);
    view(root);

    expect(root.querySelectorAll(".np-save")).toHaveLength(1);
  });

  it("shows a refusal without losing the typed name", () => {
    view(root);
    nameField(root).value = "work/github";

    showPromptError(root, "work/github is already in the store");

    const note = root.querySelector(".np-save-note") as HTMLElement;
    expect(note.hidden).toBe(false);
    expect(note.textContent).toBe("work/github is already in the store");
    expect(nameField(root).value).toBe("work/github");
  });
});
