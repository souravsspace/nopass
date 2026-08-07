/**
 * The extension's end of the native messaging port.
 *
 * The browser frames messages for us, so this layer is only responsible for
 * three things: pairing replies to requests, refusing anything off-contract in
 * either direction, and making sure no promise is left hanging when the host
 * goes away.
 *
 * The port is injected rather than imported so this can be tested without a
 * browser, and so the e2e stub can be dropped in.
 */

import {
  type ErrorCode,
  type Request,
  type ResponseFor,
  requestSchema,
  responseSchema,
  type Verb,
} from "@nopass/protocol";

/** The slice of `browser.runtime.Port` this needs. */
export interface NativePort {
  disconnect: () => void;
  onDisconnect: { addListener: (callback: () => void) => void };
  onMessage: { addListener: (callback: (message: unknown) => void) => void };
  postMessage: (message: unknown) => void;
}

/** Reasons a request can fail that are not the host's own error codes. */
export type ClientErrorCode =
  | "disconnected"
  | "timeout"
  | "unavailable"
  | "invalid_response";

/** A request minus the `id`, which the client assigns. */
export type RequestBody =
  Omit<Extract<Request, { verb: Verb }>, "id"> extends infer T ? T : never;

export class HostError extends Error {
  readonly code: ErrorCode | ClientErrorCode;

  constructor(
    code: ErrorCode | ClientErrorCode,
    message: string,
    options?: { cause?: unknown }
  ) {
    super(message, options);
    this.name = "HostError";
    this.code = code;
  }
}

interface Pending {
  reject: (error: HostError) => void;
  resolve: (value: never) => void;
  timer: ReturnType<typeof setTimeout>;
}

const DEFAULT_TIMEOUT_MS = 15_000;

export class NativeClient {
  readonly #connect: () => NativePort;
  readonly #timeoutMs: number;
  #port: NativePort | null = null;
  readonly #pending = new Map<number, Pending>();
  #nextId = 1;

  constructor(connect: () => NativePort, options: { timeoutMs?: number } = {}) {
    this.#connect = connect;
    this.#timeoutMs = options.timeoutMs ?? DEFAULT_TIMEOUT_MS;
  }

  isConnected(): boolean {
    return this.#port !== null;
  }

  /** Drop the port. Anything outstanding fails rather than hanging. */
  disconnect(): void {
    this.#port?.disconnect();
    this.#teardown("disconnected", "the connection was closed");
  }

  request<B extends { verb: Verb }>(body: B): Promise<ResponseFor<B["verb"]>> {
    // Not `async`: everything below either throws into the returned promise
    // or settles it from a listener, so there is nothing here to await.
    const id = this.#nextId++;

    // Validated before it leaves: the extension is the component most likely
    // to be tampered with, so the host should never be the first to notice.
    return new Promise<ResponseFor<B["verb"]>>((resolve, reject) => {
      const parsed = requestSchema.safeParse({ ...body, id });
      if (!parsed.success) {
        throw new HostError(
          "bad_request",
          `refusing to send an invalid request: ${parsed.error.message}`
        );
      }
      const port = this.#ensurePort();

      const timer = setTimeout(() => {
        this.#pending.delete(id);
        reject(
          new HostError(
            "timeout",
            `the host did not answer request ${id} in time`
          )
        );
      }, this.#timeoutMs);

      this.#pending.set(id, {
        reject,
        resolve: resolve as (value: never) => void,
        timer,
      });
      port.postMessage(parsed.data);
    });
  }

  #ensurePort(): NativePort {
    if (this.#port) {
      return this.#port;
    }

    let port: NativePort;
    try {
      port = this.#connect();
    } catch (cause) {
      // Chrome throws here when no manifest names this extension, which is
      // the single most likely thing to be wrong on a fresh install.
      // biome-ignore lint/style/useErrorCause: HostError forwards cause to Error; the rule only matches a literal new Error
      throw new HostError("unavailable", "could not start the nopass host", {
        cause,
      });
    }

    port.onMessage.addListener((message) => {
      this.#receive(message);
    });
    port.onDisconnect.addListener(() => {
      this.#teardown("disconnected", "the nopass host exited");
    });

    this.#port = port;
    return port;
  }

  #receive(message: unknown): void {
    const id = (message as { id?: unknown } | null)?.id;
    if (typeof id !== "number") {
      return;
    }

    const pending = this.#pending.get(id);
    // A reply for an id we never sent is noise at best; dropping it is
    // safer than guessing which request it belongs to.
    if (!pending) {
      return;
    }
    this.#pending.delete(id);
    clearTimeout(pending.timer);

    const parsed = responseSchema.safeParse(message);
    if (!parsed.success) {
      pending.reject(
        new HostError(
          "invalid_response",
          `the host replied off contract: ${parsed.error.message}`
        )
      );
      return;
    }

    const response = parsed.data;
    if (response.ok) {
      pending.resolve(response as never);
    } else {
      pending.reject(
        new HostError(response.error.code, response.error.message)
      );
    }
  }

  /** Fail everything outstanding and forget the port, so the next request
   * reconnects instead of waiting on a pipe nobody is reading. */
  #teardown(code: ClientErrorCode, message: string): void {
    this.#port = null;
    const pending = [...this.#pending.values()];
    this.#pending.clear();

    for (const entry of pending) {
      clearTimeout(entry.timer);
      entry.reject(new HostError(code, message));
    }
  }
}
