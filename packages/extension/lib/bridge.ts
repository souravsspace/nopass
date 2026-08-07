/**
 * What the popup is allowed to do, as one injectable interface.
 *
 * The real implementation talks to the background over `runtime.sendMessage`.
 * `tools/workbench` supplies a mock one, so the popup rendered there is the
 * same component with the same props, not a lookalike.
 */

import type { Match, Secret } from "@nopass/protocol";
import type { SessionState } from "./session";

export interface Bridge {
  /** The origin of the tab the popup was opened over, if it has one. */
  currentOrigin: () => Promise<string | null>;
  /** Fill the active tab with an entry. */
  fill: (entry: string) => Promise<void>;
  lock: () => Promise<SessionState>;
  /** One entry's secret, for showing or copying in the popup. */
  reveal: (entry: string) => Promise<Secret>;
  /** Entries offered for a page origin. Empty for a non-web tab. */
  search: (origin: string) => Promise<Match[]>;
  session: () => Promise<SessionState>;
  unlock: (passphrase: string) => Promise<SessionState>;
}

export class BridgeError extends Error {
  readonly code: string;

  constructor(code: string, message: string) {
    super(message);
    this.name = "BridgeError";
    this.code = code;
  }
}
