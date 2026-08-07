import type { Match } from "@nopass/protocol";
import { browser, defineContentScript } from "#imports";
import { DROPDOWN_STYLES, renderDropdown } from "../lib/dropdown";
import type { LoginForm } from "../lib/forms";
import { fillLogin, findLoginForms } from "../lib/forms";
import type {
  ContentRequest,
  ExtensionReply,
  FillCommand,
} from "../lib/messaging";
import { canFill } from "../lib/session";

export default defineContentScript({
  main() {
    let anchor: LoginForm | null = null;
    let host: HTMLDivElement | null = null;
    let root: ShadowRoot | null = null;

    const ask = (request: ContentRequest): Promise<ExtensionReply> =>
      browser.runtime.sendMessage(request) as Promise<ExtensionReply>;

    /**
     * The dropdown lives in a closed shadow root on an element the page never
     * gets a reference to. A page script can neither read what is in it nor
     * restyle it into something that looks like part of the page.
     */
    const mount = (): ShadowRoot => {
      if (root) {
        return root;
      }
      host = document.createElement("div");
      host.style.cssText =
        "all: initial; position: absolute; z-index: 2147483647;";
      root = host.attachShadow({ mode: "closed" });

      const style = document.createElement("style");
      style.textContent = DROPDOWN_STYLES;
      root.append(style);
      document.body.append(host);
      return root;
    };

    const close = () => {
      if (host) {
        host.style.display = "none";
      }
    };

    const place = (field: HTMLInputElement) => {
      if (!host) {
        return;
      }
      const box = field.getBoundingClientRect();
      host.style.display = "block";
      host.style.top = `${box.bottom + window.scrollY + 4}px`;
      host.style.left = `${box.left + window.scrollX}px`;
      host.style.width = `${Math.max(box.width, 260)}px`;
    };

    const open = async (form: LoginForm, field: HTMLInputElement) => {
      const session = await ask({ kind: "session" });
      if (!(session.ok && session.kind === "session")) {
        return;
      }

      const shadow = mount();
      anchor = form;

      if (!canFill(session.state)) {
        renderDropdown(shadow, { kind: "locked", state: session.state });
        place(field);
        return;
      }

      const reply = await ask({
        kind: "matches",
        origin: window.location.origin,
      });
      const matches: Match[] =
        reply.ok && reply.kind === "matches" ? reply.matches : [];
      if (matches.length === 0) {
        close();
        return;
      }

      renderDropdown(shadow, {
        kind: "matches",
        matches,
        onPick: (entry) => {
          close();
          void ask({ entry, kind: "fillHere" });
        },
      });
      place(field);
    };

    // The dropdown only ever opens because the user put their cursor in a
    // field. A page cannot ask for it.
    document.addEventListener(
      "focusin",
      (event) => {
        const field = event.target;
        if (!(field instanceof HTMLInputElement)) {
          return;
        }
        const form = findLoginForms(document).find(
          (candidate) =>
            candidate.password === field || candidate.username === field
        );
        if (form) {
          void open(form, field);
        }
      },
      true
    );

    document.addEventListener("focusout", () => {
      // Let a click inside the dropdown land before it disappears.
      setTimeout(close, 150);
    });
    window.addEventListener("scroll", close, { passive: true });
    window.addEventListener("resize", close, { passive: true });

    browser.runtime.onMessage.addListener((message: unknown) => {
      const command = message as FillCommand;
      if (command?.kind !== "fill") {
        return;
      }

      // The secret arrives, goes into the field, and is not kept: no module
      // variable holds it once this returns.
      const target = anchor ?? findLoginForms(document)[0];
      if (target) {
        fillLogin(target, {
          password: command.secret.password,
          username: command.secret.username,
        });
      }
      close();
    });
  },
  matches: ["http://*/*", "https://*/*"],
  runAt: "document_idle",
});
