import type { Match } from "@nopass/protocol";
import { Button } from "@nopass/ui/components/button";
import { Input } from "@nopass/ui/components/input";
import { Spinner } from "@nopass/ui/components/spinner";
import { cn } from "@nopass/ui/lib/utils";
import {
  Check,
  Copy,
  KeyRound,
  Lock,
  LockOpen,
  PlugZap,
  Search,
  SquareArrowOutUpRight,
} from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { Bridge } from "../lib/bridge";
import { displayName, folderOf } from "../lib/dropdown";
import type { SessionState } from "../lib/session";
import { initialSession } from "../lib/session";

/**
 * The popup.
 *
 * 360 by however-tall-it-needs-to-be. Everything the user came for — is it
 * unlocked, what matches this page, fill it — is reachable without scrolling
 * on a page with a handful of matches, and without a single card.
 */
export function Popup({ bridge }: { bridge: Bridge }) {
  const [session, setSession] = useState<SessionState>(initialSession);
  const [origin, setOrigin] = useState<string | null>(null);
  const [matches, setMatches] = useState<Match[]>([]);
  const [query, setQuery] = useState("");
  const [busy, setBusy] = useState(false);

  const load = useCallback(async () => {
    const [state, where] = await Promise.all([
      bridge.session(),
      bridge.currentOrigin(),
    ]);
    setSession(state);
    setOrigin(where);
    if (state.status === "unlocked" && where) {
      setMatches(await bridge.search(where));
    }
  }, [bridge]);

  useEffect(() => {
    void load();
  }, [load]);

  const visible = useMemo(() => {
    const needle = query.trim().toLowerCase();
    return needle
      ? matches.filter((m) => m.name.toLowerCase().includes(needle))
      : matches;
  }, [matches, query]);

  const onUnlock = async (passphrase: string) => {
    setBusy(true);
    try {
      setSession(await bridge.unlock(passphrase));
      await load();
    } finally {
      setBusy(false);
    }
  };

  const onLock = async () => {
    setSession(await bridge.lock());
    setMatches([]);
  };

  return (
    <div className="flex w-[360px] flex-col bg-background text-foreground">
      <Header host={hostOf(origin)} onLock={onLock} session={session} />

      {session.status === "connecting" && <Waiting />}
      {session.status === "unavailable" && (
        <Unavailable message={session.message} />
      )}
      {session.status === "no-store" && <NoStore />}
      {session.status === "locked" && (
        <Unlock busy={busy} onUnlock={onUnlock} />
      )}

      {session.status === "unlocked" && (
        <>
          <div className="relative border-b px-3 py-2.5">
            <Search className="absolute top-1/2 left-5 size-3.5 -translate-y-1/2 text-muted-foreground" />
            <Input
              autoFocus
              className="h-8 border-0 bg-muted pl-7 text-[13px] shadow-none focus-visible:ring-1"
              onChange={(event) => setQuery(event.target.value)}
              placeholder="Search entries"
              value={query}
            />
          </div>
          <EntryList
            bridge={bridge}
            entries={visible}
            hasOrigin={Boolean(origin)}
          />
        </>
      )}
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

function Header({
  host,
  session,
  onLock,
}: {
  host: string | null;
  session: SessionState;
  onLock: () => void;
}) {
  const unlocked = session.status === "unlocked";
  const Icon = unlocked ? LockOpen : Lock;

  return (
    <header className="flex items-center gap-2 border-b px-3 py-2.5">
      {/* Fill vs outline, plus the word: never hue on its own. */}
      <span
        className={cn(
          "inline-flex items-center gap-1.5 rounded-full px-2 py-0.5 font-medium text-[11px]",
          unlocked
            ? "bg-primary text-primary-foreground"
            : "border border-border text-muted-foreground"
        )}
      >
        <Icon className="size-3" />
        {unlocked ? "Unlocked" : "Locked"}
      </span>

      <span
        className="min-w-0 flex-1 truncate text-[12px] text-muted-foreground"
        title={host ?? ""}
      >
        {host ?? "No page"}
      </span>

      {unlocked && (
        <Button
          className="h-6 px-2 text-[11px]"
          onClick={onLock}
          size="sm"
          variant="ghost"
        >
          Lock
        </Button>
      )}
    </header>
  );
}

function Waiting() {
  return (
    <div className="flex items-center gap-2 px-3 py-8 text-[13px] text-muted-foreground">
      <Spinner className="size-4" />
      Connecting to nopass
    </div>
  );
}

function Unavailable({ message }: { message: string }) {
  return (
    <div className="space-y-2 px-3 py-6">
      <p className="flex items-center gap-2 font-medium text-[13px]">
        <PlugZap className="size-4 text-muted-foreground" />
        The nopass host is not reachable
      </p>
      <p className="text-[12px] text-muted-foreground leading-relaxed">
        Register it once, then reopen this popup.
      </p>
      <code
        className="block rounded-md bg-muted px-2 py-1.5 text-[11px]"
        data-secret
      >
        nopass-host install --extension-id …
      </code>
      <p className="text-[11px] text-muted-foreground">{message}</p>
    </div>
  );
}

function NoStore() {
  return (
    <div className="space-y-2 px-3 py-6">
      <p className="font-medium text-[13px]">No store yet</p>
      <p className="text-[12px] text-muted-foreground leading-relaxed">
        Create one from a terminal. The extension reads it; it never writes.
      </p>
      <code
        className="block rounded-md bg-muted px-2 py-1.5 text-[11px]"
        data-secret
      >
        nopass init
      </code>
    </div>
  );
}

function Unlock({
  busy,
  onUnlock,
}: {
  busy: boolean;
  onUnlock: (passphrase: string) => void;
}) {
  const [passphrase, setPassphrase] = useState("");

  return (
    <form
      className="space-y-2.5 px-3 py-5"
      onSubmit={(event) => {
        event.preventDefault();
        if (passphrase) {
          onUnlock(passphrase);
        }
      }}
    >
      <label
        className="flex items-center gap-2 font-medium text-[13px]"
        htmlFor="np-passphrase"
      >
        <KeyRound className="size-3.5 text-muted-foreground" />
        Passphrase
      </label>
      <Input
        autoFocus
        className="h-9"
        data-secret
        id="np-passphrase"
        onChange={(event) => setPassphrase(event.target.value)}
        type="password"
        value={passphrase}
      />
      <Button
        className="h-9 w-full"
        disabled={busy || !passphrase}
        type="submit"
      >
        {busy ? <Spinner className="size-4" /> : "Unlock"}
      </Button>
      <p className="text-[11px] text-muted-foreground leading-relaxed">
        Unlocking here also unlocks your terminal, for as long as your
        configured cache lasts.
      </p>
    </form>
  );
}

function EntryList({
  bridge,
  entries,
  hasOrigin,
}: {
  bridge: Bridge;
  entries: Match[];
  hasOrigin: boolean;
}) {
  if (entries.length === 0) {
    return (
      <p className="px-3 py-8 text-center text-[12px] text-muted-foreground">
        {hasOrigin
          ? "Nothing stored for this site."
          : "Open a site to see its logins."}
      </p>
    );
  }

  return (
    <ul className="max-h-[320px] overflow-y-auto py-1">
      {entries.map((entry) => (
        <EntryRow bridge={bridge} entry={entry} key={entry.name} />
      ))}
    </ul>
  );
}

function EntryRow({ bridge, entry }: { bridge: Bridge; entry: Match }) {
  const [copied, setCopied] = useState(false);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(
    () => () => {
      if (timer.current !== null) {
        clearTimeout(timer.current);
      }
    },
    []
  );

  const copy = async () => {
    const secret = await bridge.reveal(entry.name);
    await navigator.clipboard.writeText(secret.password);
    setCopied(true);
    timer.current = setTimeout(() => setCopied(false), 1400);
  };

  const folder = folderOf(entry.name);

  return (
    <li className="group flex items-center gap-2 px-3 py-1.5 hover:bg-accent">
      <button
        className="min-w-0 flex-1 text-left"
        onClick={() => void bridge.fill(entry.name)}
        type="button"
      >
        <span className="block truncate text-[13px]">
          {entry.username ?? displayName(entry.name)}
        </span>
        <span
          className="block truncate text-[11px] text-muted-foreground"
          data-secret
        >
          {entry.username ? entry.name : (folder ?? "")}
        </span>
      </button>

      <Button
        aria-label={`Copy the password for ${entry.name}`}
        className="size-7 opacity-0 focus-visible:opacity-100 group-hover:opacity-100"
        onClick={() => void copy()}
        size="icon"
        variant="ghost"
      >
        {copied ? (
          <Check className="size-3.5" />
        ) : (
          <Copy className="size-3.5" />
        )}
      </Button>
      <Button
        aria-label={`Fill ${entry.name} into the page`}
        className="size-7 opacity-0 focus-visible:opacity-100 group-hover:opacity-100"
        onClick={() => void bridge.fill(entry.name)}
        size="icon"
        variant="ghost"
      >
        <SquareArrowOutUpRight className="size-3.5" />
      </Button>
    </li>
  );
}
