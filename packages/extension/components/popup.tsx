import {
  MAX_PASSWORD_LENGTH,
  type Match,
  MIN_PASSWORD_LENGTH,
  type Secret,
} from "@nopass/protocol";
import { Button } from "@nopass/ui/components/button";
import { Input } from "@nopass/ui/components/input";
import { Spinner } from "@nopass/ui/components/spinner";
import { cn } from "@nopass/ui/lib/utils";
import {
  ArrowLeft,
  Check,
  ChevronRight,
  Copy,
  Database,
  Dices,
  Eye,
  EyeOff,
  Lock,
  LockOpen,
  Plus,
  Search,
  Shield,
  Unplug,
} from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { Bridge } from "../lib/bridge";
import { BridgeError } from "../lib/bridge";
import { displayName, folderOf } from "../lib/dropdown";
import type { SessionState } from "../lib/session";
import { initialSession } from "../lib/session";

/** How long the "copied" / "filled" note stays up. */
const TOAST_MS = 2000;

/** What the generator offers before anyone touches the box. */
const DEFAULT_PASSWORD_LENGTH = 20;

/**
 * Which screen the popup is on.
 *
 * Only ever one at a time and never a stack: at 360 × 556 there is nowhere to
 * go that is more than one step from the list, and a back button that
 * sometimes means two different things is worse than no history at all.
 */
type View =
  | { kind: "list" }
  | { kind: "detail"; entry: Match }
  | { kind: "new" };

/**
 * The popup.
 *
 * 360 × 556, fixed: the list scrolls, the header and the footer do not. Everything the user came for — is it unlocked, for how much longer,
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
  const [view, setView] = useState<View>({ kind: "list" });

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
      // A screen that outlived the lease would sit there holding a secret it
      // is no longer entitled to fetch again.
      setView({ kind: "list" });
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
    setView({ kind: "list" });
  };

  const onSaved = useCallback(
    async (name: string) => {
      setView({ kind: "list" });
      flash(`${displayName(name)} added.`);
      await load();
    },
    [flash, load]
  );

  const host = hostOf(origin);
  const unlocked = session.status === "unlocked";

  return (
    <div className="flex h-[556px] w-[360px] flex-col overflow-hidden bg-popover font-sans text-foreground">
      <Header
        onLock={onLock}
        onNew={
          unlocked && view.kind === "list"
            ? () => setView({ kind: "new" })
            : null
        }
        remaining={remaining}
        session={session}
      />

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

      {unlocked && view.kind === "list" && (
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
            onOpen={(entry) => setView({ entry, kind: "detail" })}
            others={visibleOthers}
          />
        </>
      )}

      {unlocked && view.kind === "detail" && (
        <Detail
          bridge={bridge}
          entry={view.entry}
          onBack={() => setView({ kind: "list" })}
          onFill={onFill}
          onFlash={flash}
        />
      )}

      {unlocked && view.kind === "new" && (
        <NewLogin
          bridge={bridge}
          host={host}
          onBack={() => setView({ kind: "list" })}
          onSaved={onSaved}
          origin={origin}
        />
      )}

      <Footer
        keys={unlocked && view.kind === "list"}
        toast={toast}
        unlocked={unlocked}
      />
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
  onNew,
}: {
  session: SessionState;
  remaining: number;
  onLock: () => void;
  /** Null on every screen that is not the list, so there is one way in. */
  onNew: (() => void) | null;
}) {
  return (
    <header className="flex flex-none items-center gap-2.5 border-b px-3.5 py-3">
      <span className="inline-flex size-6 items-center justify-center rounded-sm bg-primary font-display font-medium text-[15px] text-primary-foreground leading-none">
        n
      </span>
      <span className="flex-1 font-display font-medium text-[17px] leading-none tracking-[-0.01em]">
        nopass
      </span>

      {onNew ? (
        <Button
          aria-label="New login"
          className="size-7"
          onClick={onNew}
          size="icon"
          variant="ghost"
        >
          <Plus className="size-4" />
        </Button>
      ) : null}

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
  onOpen,
}: {
  matches: Match[];
  others: Match[];
  host: string | null;
  cursor: number;
  onFill: (entry: Match) => Promise<void>;
  onCopy: (entry: Match) => Promise<void>;
  onHover: (index: number) => void;
  onOpen: (entry: Match) => void;
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
      onOpen={onOpen}
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
  onOpen,
}: {
  entry: Match;
  index: number;
  cursor: number;
  onFill: (entry: Match) => Promise<void>;
  onCopy: (entry: Match) => Promise<void>;
  onHover: (index: number) => void;
  onOpen: (entry: Match) => void;
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

      {/* Its own control rather than the row itself: clicking a row fills,
          which is what the popup is opened for nine times out of ten. */}
      <Button
        aria-label={`View ${entry.name}`}
        className="size-7 flex-none"
        onClick={() => onOpen(entry)}
        size="icon"
        variant="ghost"
      >
        <ChevronRight className="size-3.5" />
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

/** The bar every screen that is not the list sits under. */
function SubHeader({
  title,
  meta,
  onBack,
}: {
  title: string;
  meta?: string;
  onBack: () => void;
}) {
  return (
    <div className="flex flex-none items-center gap-2 border-b px-2 py-2">
      <Button
        aria-label="Back"
        className="size-7 flex-none"
        onClick={onBack}
        size="icon"
        variant="ghost"
      >
        <ArrowLeft className="size-4" />
      </Button>
      <div className="min-w-0 flex-1">
        <span className="block truncate font-semibold text-[13px]">
          {title}
        </span>
        {meta ? (
          <span className="mt-0.5 block truncate font-mono text-[11px] text-muted-foreground">
            {meta}
          </span>
        ) : null}
      </div>
    </div>
  );
}

/** One labelled value, with the copy button that is the point of showing it. */
function DetailField({
  label,
  value,
  secret = false,
  onCopy,
}: {
  label: string;
  value: string;
  secret?: boolean;
  onCopy: () => void;
}) {
  const [shown, setShown] = useState(false);
  const masked = secret && !shown;
  const display = masked ? "••••••••••••" : value;

  return (
    <div className="flex flex-col gap-1">
      <span className="font-medium text-[11px] text-muted-foreground uppercase tracking-[0.06em]">
        {label}
      </span>
      <div className="flex items-center gap-1.5 rounded-md border bg-muted px-2.5 py-2">
        <span
          className={cn(
            "min-w-0 flex-1 truncate text-[12px]",
            secret && "font-mono"
          )}
          data-secret={secret ? "" : undefined}
        >
          {display}
        </span>
        {secret ? (
          <Button
            aria-label={shown ? `Hide the ${label}` : `Show the ${label}`}
            className="size-6 flex-none"
            onClick={() => setShown((was) => !was)}
            size="icon"
            variant="ghost"
          >
            {shown ? (
              <EyeOff className="size-3.5" />
            ) : (
              <Eye className="size-3.5" />
            )}
          </Button>
        ) : null}
        <Button
          aria-label={`Copy the ${label}`}
          className="size-6 flex-none"
          onClick={onCopy}
          size="icon"
          variant="ghost"
        >
          <Copy className="size-3.5" />
        </Button>
      </div>
    </div>
  );
}

/**
 * One entry, in full.
 *
 * Opening this screen is a `get`, because a row from "All items" carries
 * nothing but a name — `list` never returns a field. The password arrives with
 * it but stays masked until asked for: what the popup holds and what is on
 * screen over someone's shoulder are two different things.
 */
function Detail({
  bridge,
  entry,
  onBack,
  onFill,
  onFlash,
}: {
  bridge: Bridge;
  entry: Match;
  onBack: () => void;
  onFill: (entry: Match) => Promise<void>;
  onFlash: (note: string) => void;
}) {
  const [secret, setSecret] = useState<Secret | null>(null);
  const [failed, setFailed] = useState<string | null>(null);

  useEffect(() => {
    let live = true;
    bridge
      .reveal(entry.name)
      .then((found) => {
        if (live) {
          setSecret(found);
        }
      })
      .catch((error: unknown) => {
        if (live) {
          setFailed(error instanceof Error ? error.message : String(error));
        }
      });
    return () => {
      live = false;
    };
  }, [bridge, entry.name]);

  const copy = (label: string, value: string) => {
    void navigator.clipboard.writeText(value);
    onFlash(`${label} copied.`);
  };

  return (
    <>
      <SubHeader
        meta={folderOf(entry.name) ?? entry.name}
        onBack={onBack}
        title={displayName(entry.name)}
      />

      <div className="flex flex-1 flex-col gap-3 overflow-y-auto px-3.5 py-3.5">
        {failed !== null && (
          <p className="text-[12px] text-warning leading-normal" role="alert">
            {failed}
          </p>
        )}

        {secret === null && failed === null && (
          <div className="flex flex-1 items-center justify-center">
            <Spinner className="size-4 text-muted-foreground" />
          </div>
        )}

        {secret === null ? null : (
          <>
            {secret.username ? (
              <DetailField
                label="Username"
                onCopy={() => copy("Username", secret.username ?? "")}
                value={secret.username}
              />
            ) : null}
            <DetailField
              label="Password"
              onCopy={() => copy("Password", secret.password)}
              secret
              value={secret.password}
            />
            {secret.url ? (
              <DetailField
                label="URL"
                onCopy={() => copy("URL", secret.url ?? "")}
                value={secret.url}
              />
            ) : null}
            {secret.totp ? (
              <DetailField
                label="TOTP"
                onCopy={() => copy("TOTP", secret.totp ?? "")}
                secret
                value={secret.totp}
              />
            ) : null}

            <Button
              className="mt-1 w-full font-semibold"
              onClick={() => void onFill(entry)}
              type="button"
            >
              Fill this page
            </Button>
          </>
        )}
      </div>
    </>
  );
}

/**
 * A new login.
 *
 * The one screen that changes the store. It can only ever create: a name
 * already taken comes back from the host as `exists` and is shown against the
 * name field, rather than replacing what is there (ADR-0006).
 */
function NewLogin({
  bridge,
  host,
  onBack,
  onSaved,
  origin,
}: {
  bridge: Bridge;
  host: string | null;
  onBack: () => void;
  onSaved: (entry: string) => Promise<void>;
  origin: string | null;
}) {
  const [name, setName] = useState(host ? `web/${host}` : "");
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [url, setUrl] = useState(origin ?? "");
  const [shown, setShown] = useState(false);
  const [busy, setBusy] = useState(false);
  const [refused, setRefused] = useState<string | null>(null);
  const [taken, setTaken] = useState(false);
  const [length, setLength] = useState(DEFAULT_PASSWORD_LENGTH);
  const [symbols, setSymbols] = useState(true);

  /**
   * A password nobody has to think of.
   *
   * The host makes it, from the same generator `nopass generate` uses, and it
   * is shown rather than masked: this is the one moment the user has to be
   * able to read what they are about to be committed to.
   */
  const onGenerate = async () => {
    setRefused(null);
    try {
      setPassword(await bridge.generate(length, symbols));
      setShown(true);
    } catch (error) {
      setRefused(error instanceof Error ? error.message : String(error));
    }
  };

  const onSubmit = async (event: React.FormEvent) => {
    event.preventDefault();
    setBusy(true);
    setRefused(null);
    setTaken(false);
    try {
      // Omitted rather than sent empty: the host writes no line for a field
      // it was not given, so a blank one leaves no `url:` in the file at all.
      const saved = await bridge.save({
        entry: name.trim(),
        password,
        ...(url.trim() ? { url: url.trim() } : {}),
        ...(username.trim() ? { username: username.trim() } : {}),
      });
      await onSaved(saved);
    } catch (error) {
      setTaken(error instanceof BridgeError && error.code === "exists");
      setRefused(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(false);
    }
  };

  return (
    <>
      <SubHeader onBack={onBack} title="New login" />

      <form
        className="flex flex-1 flex-col gap-3 overflow-y-auto px-3.5 py-3.5"
        onSubmit={onSubmit}
      >
        <Field
          hint="Where it lands in the store. A name ending in the site's host is what makes it match."
          id="np-name"
          invalid={taken}
          label="Name"
          onChange={setName}
          placeholder="web/example.com"
          value={name}
        />
        <Field
          id="np-username"
          label="Email or username"
          onChange={setUsername}
          placeholder="you@example.com"
          value={username}
        />

        <div className="flex flex-col gap-1.5">
          <label
            className="font-medium text-[12px] text-muted-foreground"
            htmlFor="np-new-password"
          >
            Password
          </label>
          <div className="relative">
            <Input
              className="pr-9"
              data-secret
              id="np-new-password"
              onChange={(event) => setPassword(event.target.value)}
              placeholder="••••••••••••"
              type={shown ? "text" : "password"}
              value={password}
            />
            <Button
              aria-label={shown ? "Hide the password" : "Show the password"}
              className="absolute top-1/2 right-1 size-7 -translate-y-1/2"
              onClick={() => setShown((was) => !was)}
              size="icon"
              type="button"
              variant="ghost"
            >
              {shown ? (
                <EyeOff className="size-3.5" />
              ) : (
                <Eye className="size-3.5" />
              )}
            </Button>
          </div>

          <GeneratorRow
            length={length}
            onGenerate={() => void onGenerate()}
            onLength={setLength}
            onSymbols={setSymbols}
            symbols={symbols}
          />
        </div>

        <Field
          id="np-url"
          label="Website"
          onChange={setUrl}
          placeholder="https://example.com"
          value={url}
        />

        {refused !== null && (
          <p className="text-[12px] text-warning leading-normal" role="alert">
            {refused}
          </p>
        )}

        <Button
          className="mt-1 w-full font-semibold"
          disabled={busy || !name.trim() || !password}
          type="submit"
        >
          {busy ? <Spinner className="size-4" /> : "Save to store"}
        </Button>

        <p className="text-[11px] text-muted-foreground leading-normal">
          Encrypted to your store's public key. Changing or removing an entry
          still happens in a terminal.
        </p>
      </form>
    </>
  );
}

/**
 * Make one up instead of thinking of one.
 *
 * The length and the symbols are the only two things worth deciding here, and
 * both are what `nopass generate` asks for, so the two ways of creating an
 * entry produce the same passwords.
 */
function GeneratorRow({
  length,
  symbols,
  onLength,
  onSymbols,
  onGenerate,
}: {
  length: number;
  symbols: boolean;
  onLength: (next: number) => void;
  onSymbols: (next: boolean) => void;
  onGenerate: () => void;
}) {
  return (
    <div className="flex items-center gap-2">
      <Button
        className="h-7 flex-none border-border-strong bg-card px-2.5 font-semibold text-[12px]"
        onClick={onGenerate}
        size="sm"
        type="button"
        variant="outline"
      >
        <Dices className="size-3.5" />
        Generate
      </Button>

      <label
        className="flex items-center gap-1.5 text-[11px] text-muted-foreground"
        htmlFor="np-length"
      >
        Length
        <input
          className="h-7 w-14 rounded-md border bg-muted px-1.5 text-center font-mono text-[11px] text-foreground"
          id="np-length"
          max={MAX_PASSWORD_LENGTH}
          min={MIN_PASSWORD_LENGTH}
          onChange={(event) => onLength(clampLength(event.target.value))}
          type="number"
          value={length}
        />
      </label>

      <label
        className="flex items-center gap-1.5 text-[11px] text-muted-foreground"
        htmlFor="np-symbols"
      >
        <input
          checked={symbols}
          className="size-3.5 accent-primary"
          id="np-symbols"
          onChange={(event) => onSymbols(event.target.checked)}
          type="checkbox"
        />
        Symbols
      </label>
    </div>
  );
}

/** Keep the length inside what the host will accept, whatever was typed. */
function clampLength(raw: string): number {
  const asked = Number.parseInt(raw, 10);
  if (Number.isNaN(asked)) {
    return DEFAULT_PASSWORD_LENGTH;
  }
  return Math.min(Math.max(asked, MIN_PASSWORD_LENGTH), MAX_PASSWORD_LENGTH);
}

function Field({
  id,
  label,
  value,
  onChange,
  placeholder,
  hint,
  invalid = false,
}: {
  id: string;
  label: string;
  value: string;
  onChange: (next: string) => void;
  placeholder: string;
  hint?: string;
  invalid?: boolean;
}) {
  return (
    <div className="flex flex-col gap-1.5">
      <label
        className="font-medium text-[12px] text-muted-foreground"
        htmlFor={id}
      >
        {label}
      </label>
      <Input
        aria-invalid={invalid}
        id={id}
        onChange={(event) => onChange(event.target.value)}
        placeholder={placeholder}
        value={value}
      />
      {hint ? (
        <span className="text-[11px] text-muted-foreground leading-normal">
          {hint}
        </span>
      ) : null}
    </div>
  );
}

function Footer({
  toast,
  unlocked,
  keys,
}: {
  toast: string | null;
  unlocked: boolean;
  /** The arrow-key hint only means anything on the list. */
  keys: boolean;
}) {
  return (
    <div className="relative flex flex-none items-center gap-2 border-t bg-muted px-3 py-2">
      <Shield className="size-3.5 flex-none text-muted-foreground" />
      <span className="flex-1 text-[11px] text-muted-foreground leading-tight">
        {unlocked
          ? "Adds new logins. Changing or removing one is a terminal job."
          : "Reads your local store. It never changes an entry it did not create."}
      </span>
      {keys ? (
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
