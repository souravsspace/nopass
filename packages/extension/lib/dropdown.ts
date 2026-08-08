/**
 * The inline dropdown that appears under a login field.
 *
 * Plain DOM rather than React: this ships inside the content script, which is
 * injected into every page the user visits, and a framework is not worth the
 * bytes or the surface for a list of rows.
 *
 * It renders into a closed shadow root, so the page cannot read it or restyle
 * it. That also means no Tailwind, hence the stylesheet below.
 */

import type { Match } from "@nopass/protocol";
import type { SessionState } from "./session";

export type DropdownView =
  | { kind: "matches"; matches: Match[]; onPick: (entry: string) => void }
  | { kind: "locked"; state: SessionState };

/** The last path segment is the part worth reading; the rest is filing. */
export function displayName(entry: string): string {
  return entry.slice(entry.lastIndexOf("/") + 1);
}

export function folderOf(entry: string): string | null {
  const cut = entry.lastIndexOf("/");
  return cut === -1 ? null : entry.slice(0, cut);
}

function messageFor(state: SessionState): string {
  switch (state.status) {
    case "no-store":
      return "No store yet. Run nopass init.";
    case "unavailable":
      return "The nopass host is not reachable.";
    case "connecting":
      return "Connecting to nopass…";
    default:
      return "nopass is locked.";
  }
}

export function renderDropdown(root: ShadowRoot, view: DropdownView): void {
  root.querySelector(".np")?.remove();

  const panel = document.createElement("div");
  panel.className = "np";
  panel.setAttribute("role", "listbox");
  panel.setAttribute("aria-label", "nopass entries");

  if (view.kind === "locked") {
    const notice = document.createElement("div");
    notice.className = "np-empty";
    notice.textContent = messageFor(view.state);
    panel.append(notice);
    root.append(panel);
    return;
  }

  for (const match of view.matches) {
    const row = document.createElement("button");
    row.type = "button";
    row.className = "np-row";
    row.setAttribute("role", "option");
    row.dataset.entry = match.name;

    const name = document.createElement("span");
    name.className = "np-name";
    name.textContent = match.username ?? displayName(match.name);

    const meta = document.createElement("span");
    meta.className = "np-meta";
    meta.textContent = match.username
      ? displayName(match.name)
      : (folderOf(match.name) ?? "");

    row.append(name, meta);
    row.addEventListener("mousedown", (event) => {
      // mousedown, not click: focusout would tear the panel down first.
      event.preventDefault();
      view.onPick(match.name);
    });
    panel.append(row);
  }

  root.append(panel);
}

/*
 * The same warm-paper palette the popup is built on, written out by hand:
 * a shadow root cannot see Tailwind. The bundled families are absent too —
 * `@font-face` is document-scoped, and reaching a page's document would mean
 * injecting a rule and a web-accessible font into every site visited. System
 * stacks are the honest trade.
 */
export const DROPDOWN_STYLES = `
:host { all: initial; }
.np {
  font: 13px/1.35 -apple-system, BlinkMacSystemFont, "Segoe UI", ui-sans-serif, sans-serif;
  color: #191919;
  background: #ffffff;
  border: 1px solid #e6e6e6;
  border-radius: 12px;
  box-shadow: 0 2px 8px rgba(20, 20, 19, 0.07), 0 12px 32px rgba(20, 20, 19, 0.12);
  overflow: hidden;
  max-height: 264px;
  overflow-y: auto;
}
.np-row {
  all: unset;
  box-sizing: border-box;
  display: flex;
  align-items: baseline;
  gap: 8px;
  width: 100%;
  padding: 8px 12px;
  cursor: default;
}
.np-row + .np-row { border-top: 1px solid rgba(230, 230, 230, 0.6); }
.np-row:hover, .np-row:focus-visible { background: #f6f5f4; }
.np-name { font-weight: 500; flex: 1 1 auto; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.np-meta {
  font-family: ui-monospace, "SF Mono", Menlo, monospace;
  font-size: 11px;
  color: #a39e98;
  flex: 0 1 auto;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.np-empty { padding: 10px 12px; color: #a39e98; }

@media (prefers-color-scheme: dark) {
  .np {
    color: #edecea;
    background: #272623;
    border-color: #33322f;
    box-shadow: 0 12px 32px rgba(0, 0, 0, 0.5);
  }
  .np-row + .np-row { border-top-color: #33322f; }
  .np-row:hover, .np-row:focus-visible { background: #2b2a28; }
  .np-meta, .np-empty { color: #84817b; }
}
`;
