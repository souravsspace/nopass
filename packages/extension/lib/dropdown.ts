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

export const DROPDOWN_STYLES = `
:host { all: initial; }
.np {
  font: 13px/1.35 ui-sans-serif, system-ui, -apple-system, "Segoe UI", sans-serif;
  color: oklch(0.22 0.008 340);
  background: oklch(0.999 0.002 340);
  border: 1px solid oklch(0.915 0.005 340);
  border-radius: 10px;
  box-shadow: 0 1px 2px oklch(0.22 0.008 340 / 6%), 0 8px 24px oklch(0.22 0.008 340 / 10%);
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
.np-row + .np-row { border-top: 1px solid oklch(0.915 0.005 340 / 60%); }
.np-row:hover, .np-row:focus-visible { background: oklch(0.955 0.019 345); }
.np-name { font-weight: 500; flex: 1 1 auto; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.np-meta {
  font-family: ui-monospace, "SF Mono", Menlo, monospace;
  font-size: 11px;
  color: oklch(0.541 0.014 340);
  flex: 0 1 auto;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.np-empty { padding: 10px 12px; color: oklch(0.541 0.014 340); }

@media (prefers-color-scheme: dark) {
  .np {
    color: oklch(0.955 0.003 340);
    background: oklch(0.206 0.008 340);
    border-color: oklch(0.955 0.003 340 / 11%);
    box-shadow: 0 8px 24px oklch(0.08 0.006 340 / 55%);
  }
  .np-row + .np-row { border-top-color: oklch(0.955 0.003 340 / 8%); }
  .np-row:hover, .np-row:focus-visible { background: oklch(0.297 0.036 345); }
  .np-meta, .np-empty { color: oklch(0.706 0.014 340); }
}
`;
