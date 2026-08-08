import { PROTOCOL_VERSION } from "@nopass/protocol";
import { browser, defineBackground } from "#imports";
import type {
  ExtensionReply,
  ExtensionRequest,
  FillCommand,
} from "../lib/messaging";
import { failure } from "../lib/messaging";
import type { NativePort } from "../lib/native";
import { HostError, NativeClient } from "../lib/native";
import type { SessionState } from "../lib/session";
import { initialSession, reduce } from "../lib/session";

/** The name the native messaging manifest registers. */
const HOST_NAME = "com.nopass.host";

export default defineBackground(() => {
  const client = new NativeClient(
    () => browser.runtime.connectNative(HOST_NAME) as unknown as NativePort
  );

  let session: SessionState = initialSession();

  const setSession = (next: SessionState) => {
    session = next;
    void paintBadge(next);
  };

  const note = (error: unknown) => {
    if (error instanceof HostError) {
      setSession(
        reduce(session, {
          code: error.code,
          message: error.message,
          type: "failed",
        })
      );
    }
  };

  /** Say hello, then ask where things stand. Both answers move the machine. */
  const refresh = async (): Promise<SessionState> => {
    try {
      const hello = await client.request({
        verb: "hello",
        version: PROTOCOL_VERSION,
      });
      setSession(reduce(session, { store: hello.store, type: "hello" }));

      if (hello.store === "ready") {
        const status = await client.request({ verb: "status" });
        setSession(
          reduce(session, {
            expiresIn: status.expiresIn,
            state: status.state,
            type: "state",
          })
        );
      }
    } catch (error) {
      note(error);
    }
    return session;
  };

  browser.runtime.onMessage.addListener(
    (message: unknown, sender): Promise<ExtensionReply> =>
      handle(message as ExtensionRequest, sender.tab?.id)
  );

  async function handle(
    request: ExtensionRequest,
    senderTabId: number | undefined
  ): Promise<ExtensionReply> {
    try {
      switch (request.kind) {
        case "session":
          return { kind: "session", ok: true, state: await refresh() };

        case "unlock": {
          const reply = await client.request({
            passphrase: request.passphrase,
            verb: "unlock",
          });
          setSession(
            reduce(session, {
              expiresIn: reply.expiresIn,
              state: reply.state,
              type: "state",
            })
          );
          return { kind: "session", ok: true, state: session };
        }

        case "lock": {
          await client.request({ verb: "lock" });
          setSession(
            reduce(session, { expiresIn: 0, state: "locked", type: "state" })
          );
          return { kind: "session", ok: true, state: session };
        }

        case "list": {
          // Names only. `list` cannot return a secret, which is why the
          // popup may ask for the whole store and a content script may not.
          const reply = await client.request({ verb: "list" });
          return { entries: reply.entries, kind: "entries", ok: true };
        }

        case "search":
        case "matches": {
          const reply = await client.request({
            origin: request.origin,
            verb: "search",
          });
          return { kind: "matches", matches: reply.matches, ok: true };
        }

        case "reveal": {
          const reply = await client.request({
            entry: request.entry,
            verb: "get",
          });
          return { kind: "secret", ok: true, secret: reply.entry };
        }

        case "generate": {
          const reply = await client.request({
            length: request.length,
            symbols: request.symbols,
            verb: "generate",
          });
          return { kind: "password", ok: true, password: reply.password };
        }

        case "save": {
          // The only request that changes anything. It reaches the host from
          // the popup and from nowhere else — `ContentRequest` has no `save`,
          // so a page's script cannot put an entry into the store even if it
          // guessed the shape (ADR-0006).
          const reply = await client.request({
            entry: request.entry,
            password: request.password,
            url: request.url,
            username: request.username,
            verb: "insert",
          });
          return { entry: reply.entry, kind: "saved", ok: true };
        }

        case "fill":
          return await fill(request.entry, request.tabId);

        case "fillHere":
          // A content script may only fill the tab it is running in, so the
          // tab id comes from the sender and never from the message.
          return senderTabId === undefined
            ? failure("bad_request", "no tab to fill")
            : await fill(request.entry, senderTabId);

        default:
          return failure("bad_request", "unknown request");
      }
    } catch (error) {
      note(error);
      if (error instanceof HostError) {
        return failure(error.code, error.message);
      }
      return failure(
        "internal",
        error instanceof Error ? error.message : String(error)
      );
    }
  }

  /**
   * Fetch one secret and hand it straight to the page's content script.
   *
   * The secret is not stored, cached or logged on the way through: it exists
   * for the length of this call and then only inside the field it was written
   * into.
   */
  async function fill(entry: string, tabId: number): Promise<ExtensionReply> {
    const reply = await client.request({ entry, verb: "get" });
    const command: FillCommand = { kind: "fill", secret: reply.entry };
    await browser.tabs.sendMessage(tabId, command);
    return { kind: "done", ok: true };
  }

  /** The toolbar icon is the only always-visible lock indicator. */
  async function paintBadge(state: SessionState): Promise<void> {
    const locked = state.status !== "unlocked";
    try {
      await browser.action.setBadgeText({ text: locked ? "" : "•" });
      await browser.action.setBadgeBackgroundColor({ color: "#191817" });
      await browser.action.setTitle({
        title: locked ? "nopass — locked" : "nopass — unlocked",
      });
    } catch {
      // Badge APIs are cosmetic; a browser that refuses one is not a fault.
    }
  }

  void refresh();
});
