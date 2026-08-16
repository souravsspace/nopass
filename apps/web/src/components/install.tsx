/**
 * The install line, in whichever package manager the reader already has.
 *
 * Four tabs and a copy button. This is an island because it is the one thing
 * on the page a reader interacts with; everything else is static HTML.
 */

import { useState } from "react";

const WAYS = [
  {
    command: "brew tap souravsspace/tap && brew install nopass",
    id: "brew",
    label: "Homebrew",
  },
  { command: "npm install -g nopass-cli", id: "npm", label: "npm" },
  { command: "cargo install nopass-cli", id: "cargo", label: "cargo" },
  {
    command: "nix profile install github:souravsspace/nopass",
    id: "nix",
    label: "nix",
  },
] as const;

export function Install() {
  const [picked, setPicked] = useState<string>(WAYS[0].id);
  const [copied, setCopied] = useState(false);
  const command = WAYS.find((way) => way.id === picked)?.command ?? "";

  const copy = async () => {
    await navigator.clipboard.writeText(command);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  return (
    <div className="rounded-xl border bg-card">
      <div className="flex gap-1 border-b p-1.5" role="tablist">
        {WAYS.map((way) => (
          <button
            aria-selected={picked === way.id}
            className={`flex-1 rounded-md px-3 py-1.5 font-medium text-[13px] ${
              picked === way.id
                ? "bg-muted text-foreground"
                : "text-muted-foreground hover:text-foreground"
            }`}
            key={way.id}
            onClick={() => setPicked(way.id)}
            role="tab"
            type="button"
          >
            {way.label}
          </button>
        ))}
      </div>

      <div className="flex items-center gap-3 px-4 py-3.5">
        <code className="flex-1 overflow-x-auto whitespace-nowrap font-mono text-[13px]">
          {command}
        </code>
        <button
          aria-label={`Copy the ${picked} command`}
          className="rounded-md border px-2.5 py-1 font-medium text-[12px] text-muted-foreground hover:bg-muted hover:text-foreground"
          onClick={() => void copy()}
          type="button"
        >
          {copied ? "Copied" : "Copy"}
        </button>
      </div>
    </div>
  );
}
