/**
 * The "save this login?" panel that appears after a sign-in.
 *
 * Plain DOM in a closed shadow root, for the same reasons as the dropdown: it
 * ships inside the content script, and the page must be unable to read it,
 * restyle it, or click it. The password is never in here — the background is
 * holding it — so what the panel collects is a name and an answer.
 */

export interface SavePromptView {
  /** The site the login was typed into, shown so the offer is attributable. */
  host: string;
  onDismiss: () => void;
  onSave: (entry: string) => void;
  /** The name the entry would take, which the user may rewrite. */
  suggestion: string;
  username?: string | undefined;
}

/** Render the prompt. Replaces whatever was there, so it can be re-rendered. */
export function renderSavePrompt(root: ShadowRoot, view: SavePromptView): void {
  root.querySelector(".np-save")?.remove();

  const panel = document.createElement("div");
  panel.className = "np-save";
  panel.setAttribute("role", "dialog");
  panel.setAttribute("aria-label", "Save this login to nopass");

  const title = document.createElement("div");
  title.className = "np-save-title";
  title.textContent = "Save this login?";

  const where = document.createElement("div");
  where.className = "np-save-meta";
  where.textContent = view.username
    ? `${view.username} — ${view.host}`
    : view.host;

  const label = document.createElement("label");
  label.className = "np-save-label";
  label.textContent = "Name in your store";
  label.htmlFor = "np-save-name";

  const name = document.createElement("input");
  name.className = "np-save-input";
  name.id = "np-save-name";
  name.type = "text";
  name.value = view.suggestion;
  name.spellcheck = false;

  const note = document.createElement("div");
  note.className = "np-save-note";
  note.hidden = true;
  note.setAttribute("role", "alert");

  const actions = document.createElement("div");
  actions.className = "np-save-actions";

  const dismiss = document.createElement("button");
  dismiss.type = "button";
  dismiss.className = "np-save-button";
  dismiss.textContent = "Not now";
  dismiss.addEventListener("click", () => view.onDismiss());

  const save = document.createElement("button");
  save.type = "button";
  save.className = "np-save-button np-save-primary";
  save.textContent = "Save";
  save.addEventListener("click", () => {
    const entry = name.value.trim();
    if (entry) {
      view.onSave(entry);
    }
  });

  name.addEventListener("keydown", (event) => {
    if (event.key === "Enter") {
      event.preventDefault();
      save.click();
    }
  });

  actions.append(dismiss, save);
  panel.append(title, where, label, name, note, actions);
  root.append(panel);
}

/** Say why a save was refused, without losing what the user typed. */
export function showPromptError(root: ShadowRoot, message: string): void {
  const note = root.querySelector(".np-save-note");
  if (note instanceof HTMLElement) {
    note.textContent = message;
    note.hidden = false;
  }
}

/** The same hand-written palette the dropdown uses; a shadow root has no Tailwind. */
export const PROMPT_STYLES = `
.np-save {
  font: 13px/1.35 -apple-system, BlinkMacSystemFont, "Segoe UI", ui-sans-serif, sans-serif;
  display: flex;
  flex-direction: column;
  gap: 8px;
  width: 300px;
  padding: 14px;
  color: #191919;
  background: #ffffff;
  border: 1px solid #e6e6e6;
  border-radius: 12px;
  box-shadow: 0 2px 8px rgba(20, 20, 19, 0.07), 0 12px 32px rgba(20, 20, 19, 0.12);
}
.np-save-title { font-weight: 600; font-size: 14px; }
.np-save-meta {
  font-family: ui-monospace, "SF Mono", Menlo, monospace;
  font-size: 11px;
  color: #a39e98;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.np-save-label { font-size: 11px; color: #a39e98; }
.np-save-input {
  all: unset;
  box-sizing: border-box;
  width: 100%;
  padding: 7px 9px;
  font-family: ui-monospace, "SF Mono", Menlo, monospace;
  font-size: 12px;
  color: #191919;
  background: #f6f5f4;
  border: 1px solid #e6e6e6;
  border-radius: 8px;
}
.np-save-note { font-size: 11px; color: #a8442a; }
.np-save-actions { display: flex; justify-content: flex-end; gap: 8px; }
.np-save-button {
  all: unset;
  padding: 6px 12px;
  font-size: 12px;
  font-weight: 500;
  color: #191919;
  border: 1px solid #e6e6e6;
  border-radius: 8px;
  cursor: default;
}
.np-save-button:hover { background: #f6f5f4; }
.np-save-primary {
  color: #ffffff;
  background: #191817;
  border-color: #191817;
}
.np-save-primary:hover { background: #33322f; }

@media (prefers-color-scheme: dark) {
  .np-save {
    color: #edecea;
    background: #272623;
    border-color: #33322f;
    box-shadow: 0 12px 32px rgba(0, 0, 0, 0.5);
  }
  .np-save-meta, .np-save-label { color: #84817b; }
  .np-save-input {
    color: #edecea;
    background: #2b2a28;
    border-color: #33322f;
  }
  .np-save-note { color: #e0866a; }
  .np-save-button { color: #edecea; border-color: #33322f; }
  .np-save-button:hover { background: #2b2a28; }
  .np-save-primary {
    color: #191817;
    background: #edecea;
    border-color: #edecea;
  }
  .np-save-primary:hover { background: #ffffff; }
}
`;
