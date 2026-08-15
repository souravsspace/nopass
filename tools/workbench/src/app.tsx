import { Popup } from "@nopass/extension/components/popup";
import {
  DROPDOWN_STYLES,
  renderDropdown,
} from "@nopass/extension/lib/dropdown";
import { cn } from "@nopass/ui/lib/utils";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { ENTRIES, mockBridge, SCENARIOS, type Scenario } from "./mocks";

/**
 * The workbench.
 *
 * Two panes: the surface under test on the left at its real size, and what it
 * asked the host for on the right. Everything on the left is the extension's
 * own component, mounted over a mock bridge, so what is on screen here is what
 * ships.
 */
export function App() {
  const [scenario, setScenario] = useState<Scenario>("unlocked");
  const [dark, setDark] = useState(false);
  const [calls, setCalls] = useState<string[]>([]);

  useEffect(() => {
    document.documentElement.classList.toggle("dark", dark);
  }, [dark]);

  const log = useCallback((line: string) => {
    setCalls((previous) => [
      `${new Date().toLocaleTimeString()}  ${line}`,
      ...previous,
    ]);
  }, []);

  const bridge = useMemo(() => {
    setCalls([]);
    return mockBridge(scenario, log);
  }, [scenario, log]);

  return (
    <div className="min-h-dvh bg-background text-foreground">
      <header className="flex flex-wrap items-center gap-3 border-b px-6 py-3">
        <h1 className="font-medium text-sm">nopass workbench</h1>
        <p className="flex-1 text-muted-foreground text-xs">
          The extension's own components over a mock host
        </p>
        <button
          className="rounded-md border px-2.5 py-1 text-xs hover:bg-accent"
          onClick={() => setDark((value) => !value)}
          type="button"
        >
          {dark ? "Light" : "Dark"}
        </button>
      </header>

      <div className="grid gap-8 px-6 py-6 lg:grid-cols-[auto_minmax(0,1fr)]">
        <div className="space-y-6">
          <nav aria-label="Scenario" className="flex flex-wrap gap-1.5">
            {SCENARIOS.map((option) => (
              <button
                className={cn(
                  "rounded-full px-3 py-1 text-xs transition-colors",
                  scenario === option.id
                    ? "bg-primary text-primary-foreground"
                    : "border text-muted-foreground hover:bg-accent"
                )}
                key={option.id}
                onClick={() => setScenario(option.id)}
                title={option.note}
                type="button"
              >
                {option.label}
              </button>
            ))}
          </nav>

          <Surface label="Popup" note="360 × auto, as the browser renders it">
            <div
              className="overflow-hidden rounded-xl border shadow-sm"
              data-testid="popup-surface"
              key={scenario}
            >
              <Popup bridge={bridge} />
            </div>
          </Surface>

          <Surface
            label="Inline dropdown"
            note="Shadow DOM, anchored under a login field"
          >
            <InlineDropdown />
          </Surface>
        </div>

        <Surface
          label="Host calls"
          note="What the popup asked for, newest first"
        >
          <ol className="space-y-1 font-mono text-[11px] text-muted-foreground">
            {calls.length === 0 && <li>Nothing yet.</li>}
            {calls.map((line) => (
              <li key={line}>{line}</li>
            ))}
          </ol>
        </Surface>
      </div>
    </div>
  );
}

function Surface({
  label,
  note,
  children,
}: {
  label: string;
  note: string;
  children: React.ReactNode;
}) {
  return (
    <section className="space-y-2">
      <h2 className="font-medium text-xs">
        {label}
        <span className="ml-2 font-normal text-muted-foreground">{note}</span>
      </h2>
      {children}
    </section>
  );
}

/** The real renderer, in a real shadow root, over a fake field. */
function InlineDropdown() {
  const anchor = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const host = anchor.current;
    if (host === null) {
      return;
    }
    const root = host.shadowRoot ?? host.attachShadow({ mode: "open" });
    if (!root.querySelector("style")) {
      const style = document.createElement("style");
      style.textContent = DROPDOWN_STYLES;
      root.append(style);
    }
    renderDropdown(root, {
      kind: "matches",
      matches: ENTRIES.slice(0, 3).map((entry) => ({
        kind: entry.kind,
        name: entry.name,
        username: entry.fields.find((field) => field.key === "username")?.value,
      })),
      onPick: () => undefined,
    });
  }, []);

  return (
    <div className="w-[360px] space-y-1">
      <input
        className="w-full rounded-md border bg-background px-2.5 py-1.5 text-[13px]"
        placeholder="Password"
        readOnly
        type="password"
      />
      <div data-testid="dropdown-surface" ref={anchor} />
    </div>
  );
}
