/**
 * The popup's bridge, backed by the background script.
 *
 * Every call is one `runtime.sendMessage` round trip. Nothing is cached here:
 * the background owns the session, and a popup that remembered a secret would
 * keep it alive after the window closed.
 */

import type { Match, Secret } from "@nopass/protocol";
import { browser } from "#imports";
import type { Bridge } from "./bridge";
import { BridgeError } from "./bridge";
import type {
  ExtensionReply,
  ExtensionSuccess,
  PopupRequest,
} from "./messaging";
import type { SessionState } from "./session";

async function ask(request: PopupRequest): Promise<ExtensionSuccess> {
  const reply = (await browser.runtime.sendMessage(request)) as ExtensionReply;
  if (!reply.ok) {
    throw new BridgeError(reply.code, reply.message);
  }
  return reply;
}

async function activeTab(): Promise<{ id?: number; url?: string }> {
  const [tab] = await browser.tabs.query({ active: true, currentWindow: true });
  return tab ?? {};
}

export function browserBridge(): Bridge {
  const expect = async <K extends ExtensionSuccess["kind"]>(
    request: PopupRequest,
    kind: K
  ): Promise<Extract<ExtensionSuccess, { kind: K }>> => {
    const reply = await ask(request);
    if (reply.kind !== kind) {
      throw new BridgeError(
        "internal",
        `expected a ${kind} reply, got ${reply.kind}`
      );
    }
    return reply as Extract<ExtensionSuccess, { kind: K }>;
  };

  return {
    async currentOrigin(): Promise<string | null> {
      const { url } = await activeTab();
      if (!url) {
        return null;
      }
      try {
        const parsed = new URL(url);
        // A settings page or a local file is not a site with a login.
        return parsed.protocol === "http:" || parsed.protocol === "https:"
          ? parsed.origin
          : null;
      } catch {
        return null;
      }
    },

    async fill(entry: string): Promise<void> {
      const tab = await activeTab();
      if (tab.id === undefined) {
        throw new BridgeError("bad_request", "there is no tab to fill");
      }
      await ask({ entry, kind: "fill", tabId: tab.id });
      window.close();
    },

    async lock(): Promise<SessionState> {
      return (await expect({ kind: "lock" }, "session")).state;
    },

    async reveal(entry: string): Promise<Secret> {
      return (await expect({ entry, kind: "reveal" }, "secret")).secret;
    },

    async search(origin: string): Promise<Match[]> {
      return (await expect({ kind: "search", origin }, "matches")).matches;
    },
    async session(): Promise<SessionState> {
      return (await expect({ kind: "session" }, "session")).state;
    },

    async unlock(passphrase: string): Promise<SessionState> {
      return (await expect({ kind: "unlock", passphrase }, "session")).state;
    },
  };
}
