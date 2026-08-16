import { beforeEach, describe, expect, it, vi } from "vitest";
import type { NativePort, RequestBody } from "../lib/native";
import { HostError, NativeClient } from "../lib/native";

const OFF_CONTRACT = /contract|invalid/i;

/** A native port that records what was sent and replies on demand. */
class FakePort implements NativePort {
  sent: Record<string, unknown>[] = [];
  connected = true;
  private readonly messageListeners: ((message: unknown) => void)[] = [];
  private readonly disconnectListeners: (() => void)[] = [];

  onMessage = {
    addListener: (callback: (message: unknown) => void) => {
      this.messageListeners.push(callback);
    },
  };

  onDisconnect = {
    addListener: (callback: () => void) => {
      this.disconnectListeners.push(callback);
    },
  };

  postMessage(message: unknown) {
    this.sent.push(message as Record<string, unknown>);
  }

  disconnect() {
    this.connected = false;
    for (const listener of this.disconnectListeners) {
      listener();
    }
  }

  /** Deliver a reply as the browser would. */
  reply(message: unknown) {
    for (const listener of this.messageListeners) {
      listener(message);
    }
  }

  lastId(): number {
    const last = this.sent.at(-1);
    return last?.id as number;
  }
}

describe("NativeClient", () => {
  let port: FakePort;
  let client: NativeClient;

  beforeEach(() => {
    port = new FakePort();
    client = new NativeClient(() => port);
  });

  it("does not connect until something is asked of it", () => {
    expect(port.sent).toHaveLength(0);
    expect(client.isConnected()).toBe(false);
  });

  it("sends a request and resolves its reply", async () => {
    const pending = client.request({ verb: "list" });
    port.reply({
      entries: ["web/a"],
      id: port.lastId(),
      ok: true,
      verb: "list",
    });

    await expect(pending).resolves.toMatchObject({ entries: ["web/a"] });
  });

  it("gives each request a distinct id", async () => {
    const first = client.request({ verb: "list" });
    const second = client.request({ verb: "status" });

    const ids = port.sent.map((message) => message.id);
    expect(new Set(ids).size).toBe(2);

    port.reply({ entries: [], id: ids[0], ok: true, verb: "list" });
    port.reply({
      expiresIn: 0,
      id: ids[1],
      ok: true,
      state: "locked",
      verb: "status",
    });
    await Promise.all([first, second]);
  });

  it("resolves the request the reply belongs to, not the oldest one", async () => {
    const first = client.request({ verb: "list" });
    const second = client.request({ verb: "status" });
    const [firstId, secondId] = port.sent.map((message) => message.id);

    // Out of order on purpose: a host is free to answer whenever it can.
    port.reply({
      expiresIn: 60,
      id: secondId,
      ok: true,
      state: "unlocked",
      verb: "status",
    });
    await expect(second).resolves.toMatchObject({ expiresIn: 60 });

    port.reply({ entries: ["web/b"], id: firstId, ok: true, verb: "list" });
    await expect(first).resolves.toMatchObject({ entries: ["web/b"] });
  });

  it("rejects with the host's error code", async () => {
    const pending = client.request({ entry: "web/a", verb: "get" });
    port.reply({
      error: { code: "locked", message: "the store is locked" },
      id: port.lastId(),
      ok: false,
    });

    await expect(pending).rejects.toBeInstanceOf(HostError);
    await expect(pending).rejects.toMatchObject({ code: "locked" });
  });

  it("refuses to send a request the schema rejects", async () => {
    // A bad request must never reach the host: the extension is the one
    // component an attacker might control.
    await expect(client.request({ entry: "", verb: "get" })).rejects.toThrow();
    expect(port.sent).toHaveLength(0);
  });

  it("rejects a reply that is not on contract", async () => {
    const pending = client.request({ verb: "list" });
    port.reply({ entries: [42], id: port.lastId(), ok: true, verb: "list" });

    await expect(pending).rejects.toThrow(OFF_CONTRACT);
  });

  it("ignores a reply for an id it never sent", async () => {
    const pending = client.request({ verb: "list" });
    port.reply({ entries: ["ghost"], id: 9999, ok: true, verb: "list" });
    port.reply({
      entries: ["real"],
      id: port.lastId(),
      ok: true,
      verb: "list",
    });

    await expect(pending).resolves.toMatchObject({ entries: ["real"] });
  });

  it("fails everything outstanding when the host goes away", async () => {
    const first = client.request({ verb: "list" });
    const second = client.request({ verb: "status" });
    port.disconnect();

    await expect(first).rejects.toMatchObject({ code: "disconnected" });
    await expect(second).rejects.toMatchObject({ code: "disconnected" });
    expect(client.isConnected()).toBe(false);
  });

  it("gives up on a host that never answers", async () => {
    vi.useFakeTimers();
    const impatient = new NativeClient(() => port, { timeoutMs: 1000 });
    const pending = impatient.request({ verb: "list" });

    vi.advanceTimersByTime(1001);
    await expect(pending).rejects.toMatchObject({ code: "timeout" });
    vi.useRealTimers();
  });

  it("reconnects after a disconnect rather than staying dead", async () => {
    const ports: FakePort[] = [];
    const reconnecting = new NativeClient(() => {
      const fresh = new FakePort();
      ports.push(fresh);
      return fresh;
    });

    const first = reconnecting.request({ verb: "list" });
    const original = ports.at(0);
    original?.disconnect();
    await expect(first).rejects.toMatchObject({ code: "disconnected" });

    const second = reconnecting.request({ verb: "list" });
    expect(ports).toHaveLength(2);
    const fresh = ports.at(1);
    fresh?.reply({
      entries: ["back"],
      id: fresh.lastId(),
      ok: true,
      verb: "list",
    });
    await expect(second).resolves.toMatchObject({ entries: ["back"] });
  });

  it("reports a host that cannot be launched at all", async () => {
    const broken = new NativeClient(() => {
      throw new Error("Specified native messaging host not found.");
    });

    await expect(broken.request({ verb: "list" })).rejects.toMatchObject({
      code: "unavailable",
    });
  });
});

describe("the request type", () => {
  /*
   * These assertions are checked by `tsc`, not by vitest: the bug they exist
   * for — an `insert` still carrying the pre-v2 `password` and `url` keys —
   * reached a shipped build because `RequestBody` collapsed the request union
   * into its common keys and accepted anything. A wrong shape has to be a
   * compile error, because at runtime it is a `bad_request` from the host and
   * a save prompt that silently does nothing.
   */
  it("describes each verb's own fields", () => {
    const insert: RequestBody = {
      entry: "web/example.com",
      fields: [{ key: "username", value: "sana" }],
      kind: "login",
      secret: "hunter2",
      verb: "insert",
    };
    const update: RequestBody = {
      entry: "web/example.com",
      fields: [{ key: "username", value: "sana" }],
      verb: "update",
    };

    expect(insert.verb).toBe("insert");
    expect(update.verb).toBe("update");
  });

  it("refuses the shape the wire used before records", () => {
    const legacy: RequestBody = {
      entry: "web/example.com",
      // @ts-expect-error `password` and `url` were replaced by `secret` and
      // `fields` when entries gained kinds (ADR-0008).
      password: "hunter2",
      url: "https://example.com",
      verb: "insert",
    };

    expect(legacy.verb).toBe("insert");
  });

  it("refuses a verb that does not exist", () => {
    // @ts-expect-error `edit` is refused at the protocol layer, so there is
    // no shape of it to build.
    const nonsense: RequestBody = { entry: "web/example.com", verb: "edit" };

    expect(nonsense).toBeTruthy();
  });
});
