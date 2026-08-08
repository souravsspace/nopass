import type { Match } from "@nopass/protocol";
import { Button } from "@nopass/ui/components/button";
import { Input } from "@nopass/ui/components/input";
import { Spinner } from "@nopass/ui/components/spinner";
import { cn } from "@nopass/ui/lib/utils";
import {
  Check,
  Copy,
  Database,
  Lock,
  LockOpen,
  Search,
  Shield,
  Unplug,
} from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { Bridge } from "../lib/bridge";
import { displayName, folderOf } from "../lib/dropdown";
import type { SessionState } from "../lib/session";
import { initialSession } from "../lib/session";

/** How long the "copied" / "filled" note stays up. */
const TOAST_MS = 2000;

/**
 * The popup.
 *
 * 360 × 556, fixed: the list scrolls, the header and the read-only footer do
 * not. Everything the user came for — is it unlocked, for how much longer,
 * what matches this page, fill it — is on screen without scrolling.
 *
 * Two sections, and they are two different verbs. "This page" is `search`.
 * "All items" is `list`, which returns names and never a secret, so offering
 * the whole store here costs nothing that was not already on disk.
 */
export function Popup({ bridge }: { bridge: Bridge }) {
  const [session, setSession] = useState<SessionState>(initialSession);
  const [origin, setOrigin] = useState<string | null>(null);
  const [matches, setMatches] = useState<Match[]>([]);
  const [others, setOthers] = useState<Match[]>([]);
  const [query, setQuery] = useState("");
  const [cursor, setCursor] = useState(0);
  const [busy, setBusy] = useState(false);
  const [refused, setRefused] = useState<string | null>(null);
  const [toast, setToast] = useState<string | null>(null);
  const [remaining, setRemaining] = useState(0);

  const toastTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(
    () => () => {
      if (toastTimer.current !== null) {
        clearTimeout(toastTimer.current);
      }
    },
    []
  );

  const flash = useCallback((message: string) => {
    if (toastTimer.current !== null) {
      clearTimeout(toastTimer.current);
    }
    setToast(message);
    toastTimer.current = setTimeout(() => setToast(null), TOAST_MS);
  }, []);

  const load = useCallback(async () => {
    const [state, where] = await Promise.all([
      bridge.session(),
      bridge.currentOrigin(),
    ]);
    setSession(state);
    setOrigin(where);

    if (state.status !== "unlocked") {
      setMatches([]);
      setOthers([]);
      return;
    }

    const [found, names] = await Promise.all([
      where ? bridge.search(where) : Promise.resolve<Match[]>([]),
      bridge.list(),
    ]);
    const taken = new Set(found.map((entry) => entry.name));
    setMatches(found);
    setOthers(
      names.filter((name) => !taken.has(name)).map((name) => ({ name }))
    );
  }, [bridge]);

  useEffect(() => {
    void load();
  }, [load]);

  /*
   * The countdown is a copy of the agent's TTL, not a timer this popup owns.
   *
   * It is read off the wall clock rather than decremented, for the same reason
   * the agent expires on `SystemTime`: a laptop that sleeps for ten minutes has
   * to wake up ten minutes closer to locked, not one tick closer. At zero the
   * host is asked instead of assumed — only it knows whether something else
   * renewed the lease (ADR-0005) — and the interval stops, so that costs one
   * round trip per expiry rather than a poll.
   */
  useEffect(() => {
    if (session.status !== "unlocked" || session.expiresIn <= 0) {
      return;
    }
    const deadline = Date.now() + session.expiresIn * 1000;
    setRemaining(session.expiresIn);

    const timer = setInterval(() => {
      const left = Math.round((deadline - Date.now()) / 1000);
      setRemaining(Math.max(0, left));
      if (left <= 0) {
        clearInterval(timer);
        void load();
      }
    }, 1000);
    return () => clearInterval(timer);
  }, [session, load]);

  const { visibleMatches, visibleOthers } = useMemo(() => {
    const needle = query.trim().toLowerCase();
    const keep = (entry: Match) =>
      !needle || entry.name.toLowerCase().includes(needle);
    return {
      visibleMatches: matches.filter(keep),
      visibleOthers: others.filter(keep),
    };
  }, [matches, others, query]);

  // One selection shared by the mouse and the arrow keys, so hovering a row
  // and pressing Enter cannot disagree about which entry is meant.
  const flat = useMemo(
    () => [...visibleMatches, ...visibleOthers],
    [visibleMatches, visibleOthers]
  );

  const onFill = useCallback(
    async (entry: Match) => {
      // The real bridge closes the window on the way out, so this note is for
      // the workbench and for a browser that declines to close.
      flash(`Filled into ${hostOf(origin) ?? "the page"}`);
      await bridge.fill(entry.name);
    },
    [bridge, flash, origin]
  );

  const onCopy = useCallback(
    async (entry: Match) => {
      const secret = await bridge.reveal(entry.name);
      await navigator.clipboard.writeText(secret.password);
      flash(`Password for ${displayName(entry.name)} copied.`);
    },
    [bridge, flash]
  );

  const onKeys = (event: React.KeyboardEvent<HTMLInputElement>) => {
    if (flat.length === 0) {
      return;
    }
    switch (event.key) {
      case "ArrowDown":
        event.preventDefault();
        setCursor((at) => (at + 1) % flat.length);
        break;
      case "ArrowUp":
        event.preventDefault();
        setCursor((at) => (at - 1 + flat.length) % flat.length);
        break;
      case "Enter": {
        event.preventDefault();
        const picked = flat[Math.min(cursor, flat.length - 1)];
        if (picked) {
          void onFill(picked);
        }
        break;
      }
      default:
        break;
    }
  };

  // A refusal is the whole answer here: the host says whether the passphrase
  // was wrong or whether it could not hold the unlock, and a popup that
  // swallowed that would look exactly the same either way.
  const onUnlock = async (passphrase: string) => {
    setBusy(true);
    setRefused(null);
    try {
      setSession(await bridge.unlock(passphrase));
      await load();
    } catch (error) {
      setRefused(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(false);
    }
  };

  const onLock = async () => {
    setSession(await bridge.lock());
    setMatches([]);
    setOthers([]);
    setRemaining(0);
  };

  const host = hostOf(origin);

  return (
    <div className="flex h-[556px] w-[360px] flex-col overflow-hidden bg-popover font-sans text-foreground">
      <Header onLock={onLock} remaining={remaining} session={session} />

      {session.status === "connecting" && <Connecting />}
      {session.status === "unavailable" && (
        <Unavailable message={session.message} onCopied={flash} />
      )}
      {session.status === "no-store" && <NoStore onCopied={flash} />}
      {session.status === "locked" && (
        <Unlock
          busy={busy}
          host={host}
          onEdit={() => setRefused(null)}
          onUnlock={onUnlock}
          refused={refused}
        />
      )}

      {session.status === "unlocked" && (
        <>
          <div className="relative flex-none border-b px-3 py-2.5">
            <Search className="absolute top-1/2 left-6 size-3.5 -translate-y-1/2 text-muted-foreground" />
            <Input
              autoFocus
              className="h-9 border-transparent bg-muted pl-8.5 text-[13px] shadow-none"
              onChange={(event) => {
                setQuery(event.target.value);
                setCursor(0);
              }}
              onKeyDown={onKeys}
              placeholder="Search your store"
              value={query}
            />
          </div>
          <EntryList
            cursor={cursor}
            host={host}
            matches={visibleMatches}
            onCopy={onCopy}
            onFill={onFill}
            onHover={setCursor}
            others={visibleOthers}
          />
        </>
      )}

      <Footer toast={toast} unlocked={session.status === "unlocked"} />
    </div>
  );
}

function hostOf(origin: string | null): string | null {
  if (!origin) {
    return null;
  }
  try {
    return new URL(origin).host;
  } catch {
    return null;
  }
}

/** `5:00`, so the pill is the same width for most of the lease. */
function clock(seconds: number): string {
  const minutes = Math.floor(seconds / 60);
  return `${minutes}:${String(seconds % 60).padStart(2, "0")}`;
}

function initialOf(entry: string): string {
  return displayName(entry).charAt(0).toUpperCase();
}

function Header({
  session,
  remaining,
  onLock,
}: {
  session: SessionState;
  remaining: number;
  onLock: () => void;
}) {
  return (
    <header className="flex flex-none items-center gap-2.5 border-b px-3.5 py-3">
      <span className="inline-flex size-6 items-center justify-center rounded-sm bg-primary font-display font-medium text-[15px] text-primary-foreground leading-none">
        n
      </span>
      <span className="flex-1 font-display font-medium text-[17px] leading-none tracking-[-0.01em]">
        nopass
      </span>

      {/* Fill vs outline, plus the word or the clock: never hue on its own. */}
      {session.status === "unlocked" ? (
        <button
          aria-label="Lock now"
          className="inline-flex items-center gap-1.5 rounded-full bg-primary px-2.5 py-1 font-semibold text-[11px] text-primary-foreground hover:bg-primary-hover"
          onClick={onLock}
          type="button"
        >
          <LockOpen className="size-3" />
          <span className="font-mono font-normal tabular-nums">
            {clock(remaining)}
          </span>
        </button>
      ) : (
        <span className="inline-flex items-center gap-1.5 rounded-full border border-border-strong px-2.5 py-1 font-medium text-[11px] text-muted-foreground">
          <Lock className="size-3" />
          Locked
        </span>
      )}
    </header>
  );
}

function Connecting() {
  return (
    <div className="flex flex-1 flex-col items-center justify-center gap-3 px-6">
      <Spinner className="size-5 text-muted-foreground" />
      <span className="text-[13px] text-muted-foreground">
        Connecting to nopass
      </span>
      <span className="font-mono text-[11px] text-muted-foreground">
        com.nopass.host
      </span>
    </div>
  );
}

function Unavailable({
  message,
  onCopied,
}: {
  message: string;
  onCopied: (note: string) => void;
}) {
  return (
    <div className="flex flex-1 flex-col justify-center gap-3 px-5 py-6">
      <span className="inline-flex size-10 items-center justify-center rounded-md bg-warning-subtle text-warning">
        <Unplug className="size-5" />
      </span>
      <span className="font-semibold text-[15px]">
        The nopass host is not reachable
      </span>
      <p className="text-[13px] text-muted-foreground leading-relaxed">
        Register it once from a terminal, then reopen this popup.
      </p>
      <CommandRow
        command="nopass-host install --extension-id …"
        onCopied={onCopied}
      />
      <p className="font-mono text-[11px] text-muted-foreground leading-normal">
        {message}
      </p>
    </div>
  );
}

function NoStore({ onCopied }: { onCopied: (note: string) => void }) {
  return (
    <div className="flex flex-1 flex-col justify-center gap-3 px-5 py-6">
      <span className="inline-flex size-10 items-center justify-center rounded-md bg-muted text-foreground">
        <Database className="size-5" />
      </span>
      <span className="font-semibold text-[15px]">No store yet</span>
      <p className="text-[13px] text-muted-foreground leading-relaxed">
        Create one from a terminal. The extension reads it; it never writes.
      </p>
      <CommandRow command="nopass init" onCopied={onCopied} />
    </div>
  );
}

function CommandRow({
  command,
  onCopied,
}: {
  command: string;
  onCopied: (note: string) => void;
}) {
  return (
    <div className="flex items-center gap-2 rounded-md border bg-muted px-2.5 py-2">
      <code className="flex-1 overflow-x-auto whitespace-nowrap font-mono text-[11px]">
        {command}
      </code>
      <Button
        aria-label={`Copy ${command}`}
        className="size-6 flex-none"
        onClick={() => {
          void navigator.clipboard.writeText(command);
          onCopied("Command copied.");
        }}
        size="icon"
        variant="ghost"
      >
        <Copy className="size-3.5" />
      </Button>
    </div>
  );
}

function Unlock({
  busy,
  host,
  onEdit,
  onUnlock,
  refused,
}: {
  busy: boolean;
  host: string | null;
  onEdit: () => void;
  onUnlock: (passphrase: string) => void;
  refused: string | null;
}) {
  const [passphrase, setPassphrase] = useState("");

  return (
    <form
      className="flex flex-1 flex-col justify-center gap-3.5 px-5 py-6"
      onSubmit={(event) => {
        event.preventDefault();
        if (passphrase) {
          onUnlock(passphrase);
        }
      }}
    >
      <span className="inline-flex size-11 items-center justify-center rounded-full bg-muted">
        <Lock className="size-5" />
      </span>

      <div className="flex flex-col gap-1">
        <span className="font-display font-medium text-[20px] leading-tight">
          Your store is locked
        </span>
        <span className="text-[13px] text-muted-foreground leading-normal">
          {host ? (
            <>
              Unlock to fill on{" "}
              <span className="font-mono text-[11px]">{host}</span>.
            </>
          ) : (
            "Unlock to reach your entries."
          )}
        </span>
      </div>

      <div className="flex flex-col gap-1.5">
        <label
          className="font-medium text-[12px] text-muted-foreground"
          htmlFor="np-passphrase"
        >
          Master passphrase
        </label>
        <Input
          aria-invalid={refused !== null}
          autoFocus
          data-secret
          id="np-passphrase"
          onChange={(event) => {
            setPassphrase(event.target.value);
            onEdit();
          }}
          placeholder="••••••••••••"
          type="password"
          value={passphrase}
        />
        {refused !== null && (
          <p className="text-[12px] text-warning leading-normal" role="alert">
            {refused}
          </p>
        )}
      </div>

      <Button
        className="w-full font-semibold"
        disabled={busy || !passphrase}
        type="submit"
      >
        {busy ? <Spinner className="size-4" /> : "Unlock"}
      </Button>

      <p className="text-[11px] text-muted-foreground leading-normal">
        Unlocking here also unlocks your terminal, for as long as your
        configured cache lasts. Warm the agent from a terminal to skip this step
        entirely.
      </p>
    </form>
  );
}

function EntryList({
  matches,
  others,
  host,
  cursor,
  onFill,
  onCopy,
  onHover,
}: {
  matches: Match[];
  others: Match[];
  host: string | null;
  cursor: number;
  onFill: (entry: Match) => Promise<void>;
  onCopy: (entry: Match) => Promise<void>;
  onHover: (index: number) => void;
}) {
  const row = (entry: Match, index: number) => (
    <EntryRow
      cursor={cursor}
      entry={entry}
      index={index}
      key={entry.name}
      onCopy={onCopy}
      onFill={onFill}
      onHover={onHover}
    />
  );

  return (
    <div className="flex-1 overflow-y-auto px-1.5 pt-1.5 pb-2.5 [&::-webkit-scrollbar-thumb]:rounded-full [&::-webkit-scrollbar-thumb]:border-[3px] [&::-webkit-scrollbar-thumb]:border-transparent [&::-webkit-scrollbar-thumb]:bg-border-strong [&::-webkit-scrollbar-thumb]:bg-clip-content [&::-webkit-scrollbar]:w-2.5">
      {matches.length > 0 && (
        <>
          <SectionLabel meta={host ?? ""} title="This page" />
          <ul aria-label="Entries for this page">{matches.map(row)}</ul>
        </>
      )}

      {/* Only worth saying when there is a site to have nothing stored for. */}
      {host !== null && matches.length === 0 && others.length > 0 && (
        <div className="flex flex-col items-center gap-1.5 px-5 pt-7 pb-5 text-center">
          <Search className="size-4.5 text-muted-foreground" />
          <span className="font-semibold text-[13px]">
            Nothing stored for this site.
          </span>
          <span className="text-[12px] text-muted-foreground leading-normal">
            Entries match on their{" "}
            <span className="font-mono text-[11px]">url:</span> line, or on a
            name that ends in the host.
          </span>
        </div>
      )}

      {others.length > 0 && (
        <>
          <SectionLabel
            bordered
            meta={String(others.length)}
            title="All items"
          />
          <ul aria-label="All entries">
            {others.map((entry, index) => row(entry, matches.length + index))}
          </ul>
        </>
      )}

      {matches.length === 0 && others.length === 0 && (
        <div className="flex flex-col items-center gap-1.5 px-5 py-9 text-center">
          <span className="font-semibold text-[13px]">
            No entry matches that.
          </span>
          <span className="text-[12px] text-muted-foreground leading-normal">
            Search runs over names, before anything is decrypted.
          </span>
        </div>
      )}
    </div>
  );
}

function SectionLabel({
  title,
  meta,
  bordered = false,
}: {
  title: string;
  meta: string;
  bordered?: boolean;
}) {
  return (
    <div
      className={cn(
        "flex items-center gap-2 px-2 pt-2.5 pb-1.5",
        bordered && "mt-0.5 border-t pt-3.5"
      )}
    >
      <span className="font-semibold text-[11px] text-muted-foreground uppercase tracking-[0.08em]">
        {title}
      </span>
      <span className="truncate font-mono text-[11px] text-muted-foreground">
        {meta}
      </span>
    </div>
  );
}

function EntryRow({
  entry,
  index,
  cursor,
  onFill,
  onCopy,
  onHover,
}: {
  entry: Match;
  index: number;
  cursor: number;
  onFill: (entry: Match) => Promise<void>;
  onCopy: (entry: Match) => Promise<void>;
  onHover: (index: number) => void;
}) {
  const selected = cursor === index;
  const folder = folderOf(entry.name);

  return (
    // biome-ignore lint/a11y/noNoninteractiveElementInteractions: hover only mirrors the arrow-key cursor, and every row is already reachable with ArrowUp/ArrowDown
    <li
      className={cn(
        "flex items-center gap-2.5 rounded-md p-2",
        selected && "bg-accent shadow-[inset_0_0_0_1px_var(--border)]"
      )}
      onMouseEnter={() => onHover(index)}
    >
      <span
        className={cn(
          "inline-flex size-8 flex-none items-center justify-center rounded-md font-display font-medium text-[15px] leading-none",
          selected
            ? "bg-primary text-primary-foreground"
            : "bg-muted text-foreground"
        )}
      >
        {initialOf(entry.name)}
      </span>

      <button
        className="min-w-0 flex-1 text-left"
        onClick={() => void onFill(entry)}
        type="button"
      >
        <span className="block truncate font-semibold text-[13px]">
          {entry.username ?? displayName(entry.name)}
        </span>
        <span className="mt-0.5 block truncate font-mono text-[11px] text-muted-foreground">
          {entry.username ? entry.name : (folder ?? entry.name)}
        </span>
      </button>

      <Button
        aria-label={`Copy the password for ${entry.name}`}
        className="size-7 flex-none"
        onClick={() => void onCopy(entry)}
        size="icon"
        variant="ghost"
      >
        <Copy className="size-3.5" />
      </Button>

      {selected && (
        <Button
          aria-label={`Fill ${entry.name} into the page`}
          className="h-7 flex-none border-border-strong bg-card px-2.5 font-semibold text-[12px]"
          onClick={() => void onFill(entry)}
          size="sm"
          variant="outline"
        >
          Fill
        </Button>
      )}
    </li>
  );
}

function Footer({
  toast,
  unlocked,
}: {
  toast: string | null;
  unlocked: boolean;
}) {
  return (
    <div className="relative flex flex-none items-center gap-2 border-t bg-muted px-3 py-2">
      <Shield className="size-3.5 flex-none text-muted-foreground" />
      <span className="flex-1 text-[11px] text-muted-foreground leading-tight">
        Read-only. Add logins with{" "}
        <code className="font-mono">nopass insert</code>.
      </span>
      {unlocked ? (
        <span className="font-mono text-[11px] text-muted-foreground">
          ↑↓ ↵
        </span>
      ) : null}

      {toast === null ? null : (
        <div className="fade-in slide-in-from-bottom-1 absolute right-3 bottom-11 left-3 flex animate-in items-center gap-2 rounded-md bg-foreground px-3 py-2.5 text-background shadow-md duration-200">
          <Check className="size-3.5 flex-none" />
          <span className="text-[12px]">{toast}</span>
        </div>
      )}
    </div>
  );
}
