import { PROTOCOL_VERSION } from "@nopass/protocol";
import { browser, defineBackground } from "#imports";
import { suggestedName } from "../lib/capture";
import type {
  Captured,
  ExtensionReply,
  ExtensionRequest,
  FillCommand,
  OfferCommand,
  SaveOffer,
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

  /**
   * Logins that have been submitted and not yet answered for, by tab.
   *
   * The password sits here rather than in the page while the prompt is up, so
   * the message that accepts the offer carries a name and nothing else. It is
   * memory only: nothing is written until the user says a name, and closing
   * the tab throws it away (ADR-0007).
   *
   * `pushed` records that the offer has been put on the page the tab landed on
   * after the sign-in, so an offer the user simply ignores is asked once and
   * not again on every navigation afterwards.
   */
  const offered = new Map<
    number,
    Captured & { offer: SaveOffer; origin: string; pushed: boolean }
  >();

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

  /*
   * `sendResponse` and a literal `true`, rather than returning the promise.
   *
   * Returning a promise from a message listener is a Gecko extension that
   * Chromium has never implemented: there, the channel closes the moment this
   * listener returns and every caller resolves with `undefined`, which is why
   * nothing the popup or the content script asked for ever came back. `true`
   * says the answer is coming later, and both engines understand it.
   */
  browser.runtime.onMessage.addListener(
    (
      message: unknown,
      sender,
      sendResponse: (reply: ExtensionReply) => void
    ) => {
      handle(message as ExtensionRequest, sender.tab?.id).then(
        sendResponse,
        (error: unknown) =>
          sendResponse(
            failure(
              "internal",
              error instanceof Error ? error.message : String(error)
            )
          )
      );
      return true;
    }
  );

  // A tab that goes away takes its unanswered offer with it.
  browser.tabs.onRemoved.addListener((tabId) => {
    offered.delete(tabId);
  });

  /*
   * A sign-in that posts a form navigates, and the content script that made
   * the offer dies mid-question. The login is still here, so the offer goes to
   * whatever page the tab landed on instead.
   */
  browser.tabs.onUpdated.addListener((tabId, changes) => {
    const held = offered.get(tabId);
    if (changes.status !== "complete" || !held || held.pushed) {
      return;
    }
    held.pushed = true;
    const command: OfferCommand = { kind: "offerSave", offer: held.offer };
    // A page with no content script in it — a settings page, a PDF — is not
    // a fault; there is simply nowhere to ask.
    void browser.tabs.sendMessage(tabId, command).catch(() => undefined);
  });

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

        case "save":
        case "update":
        case "items":
        case "wallet":
          return await records(request);

        case "captured":
        case "saveCaptured":
        case "dismissCaptured":
          // The tab id comes from the sender, so a tab can only ever offer,
          // name or drop its own login (ADR-0007).
          return senderTabId === undefined
            ? failure("bad_request", "no tab offered this login")
            : await captured(request, senderTabId);

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
   * The verbs that read or write a record.
   *
   * `save` and `update` are the popup's alone — a page's script cannot reach
   * either (ADR-0006, ADR-0009) — and `wallet` is the one a content script may
   * ask, for the two kinds that belong to no site.
   */
  async function records(
    request: Extract<
      ExtensionRequest,
      { kind: "items" | "save" | "update" | "wallet" }
    >
  ): Promise<ExtensionReply> {
    switch (request.kind) {
      case "save": {
        // Creating reaches the host from the popup and from nowhere else —
        // `ContentRequest` has no `save`, so a page's script cannot put an
        // entry into the store even if it guessed the shape (ADR-0006).
        const reply = await client.request({
          entry: request.draft.entry,
          verb: "insert",
          ...(request.draft.kind ? { kind: request.draft.kind } : {}),
          ...(request.draft.secret === undefined
            ? {}
            : { secret: request.draft.secret }),
          ...(request.draft.fields ? { fields: request.draft.fields } : {}),
        });
        return { entry: reply.entry, kind: "saved", ok: true };
      }

      case "update": {
        // Rewriting is the popup's alone, behind the confirmation on the
        // entry screen (ADR-0009). It names fields; the host leaves every
        // line it was not told about exactly where it is.
        const reply = await client.request({
          entry: request.patch.entry,
          verb: "update",
          ...(request.patch.secret === undefined
            ? {}
            : { secret: request.patch.secret }),
          ...(request.patch.fields ? { fields: request.patch.fields } : {}),
        });
        return { entry: reply.entry, kind: "saved", ok: true };
      }

      case "items": {
        const reply = await client.request({
          verb: "items",
          ...(request.kinds ? { kinds: request.kinds } : {}),
        });
        return { items: reply.items, kind: "items", ok: true };
      }

      case "wallet": {
        // A content script may ask for these two kinds and no others: a
        // card and an identity belong to no site, so `search` cannot offer
        // them, and enumerating logins is what its vocabulary exists to
        // prevent.
        const reply = await client.request({
          kinds: ["card", "identity"],
          verb: "items",
        });
        return { items: reply.items, kind: "items", ok: true };
      }
      default:
        return failure("bad_request", "unknown request");
    }
  }

  /** The three steps of a login the user was asked about after signing in. */
  async function captured(
    request: Extract<
      ExtensionRequest,
      { kind: "captured" | "dismissCaptured" | "saveCaptured" }
    >,
    tabId: number
  ): Promise<ExtensionReply> {
    if (request.kind === "dismissCaptured") {
      offered.delete(tabId);
      return { kind: "done", ok: true };
    }

    if (request.kind === "captured") {
      return await offer(tabId, request.origin, {
        password: request.password,
        ...(request.username ? { username: request.username } : {}),
      });
    }

    // Only the name travels. The password is the one this tab reported when
    // the form was submitted, so accepting an offer can never save something
    // the user did not just type into that page (ADR-0007).
    const held = offered.get(tabId);
    if (!held) {
      return failure("bad_request", "there is no login to save");
    }

    const reply = await client.request({
      entry: request.entry,
      fields: [
        { key: "url", value: held.origin },
        ...(held.username ? [{ key: "username", value: held.username }] : []),
      ],
      kind: "login",
      secret: held.password,
      verb: "insert",
    });
    offered.delete(tabId);
    return { entry: reply.entry, kind: "saved", ok: true };
  }

  /**
   * Hold a submitted login, and say whether it is worth asking about.
   *
   * Nothing is asked when the store is locked — the answer would be an unlock
   * prompt over a page that is already navigating away — or when the site
   * already has an entry under that username, which is the ordinary case of
   * signing in again.
   */
  async function offer(
    tabId: number,
    origin: string,
    login: Captured
  ): Promise<ExtensionReply> {
    const host = hostOf(origin);
    if (!host) {
      return { kind: "offer", offer: null, ok: true };
    }

    const state = await refresh();
    if (state.status !== "unlocked") {
      return { kind: "offer", offer: null, ok: true };
    }

    const { matches } = await client.request({ origin, verb: "search" });
    const known = matches.some(
      (match) => (match.username ?? "") === (login.username ?? "")
    );
    if (known) {
      return { kind: "offer", offer: null, ok: true };
    }

    const details: SaveOffer = {
      host,
      suggestion: suggestedName(host),
      ...(login.username ? { username: login.username } : {}),
    };
    offered.set(tabId, { ...login, offer: details, origin, pushed: false });
    return { kind: "offer", offer: details, ok: true };
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

  /** The host of a page origin, or null for anything that is not a web page. */
  function hostOf(origin: string): string | null {
    try {
      const parsed = new URL(origin);
      return parsed.protocol === "http:" || parsed.protocol === "https:"
        ? parsed.host
        : null;
    } catch {
      return null;
    }
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
