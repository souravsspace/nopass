/**
 * The docs, as data.
 *
 * One array, rendered three ways: the page a person reads, the sidebar that
 * navigates it, and the plain-text mirror at `/docs.md` that an answer engine
 * can quote without parsing a layout. Writing it once is what keeps those
 * three from drifting.
 */

export interface Section {
  /** Markdown: paragraphs, `- ` lists, and fenced code. */
  body: string;
  id: string;
  title: string;
}

export const SECTIONS: Section[] = [
  {
    body: `nopass is a password manager whose store is a directory of individually encrypted files. There is no server, no account, and no vault format — every entry is an age ciphertext you can decrypt with any age client.

It has three parts. The \`nopass\` command-line tool, which is the whole product. The \`nopass-host\` binary, which the browser launches over native messaging. And a browser extension for Chrome, Chromium, Brave, Edge and Firefox, which fills logins, cards and addresses from the same store.

It runs on macOS and Linux.`,
    id: "what-nopass-is",
    title: "What nopass is",
  },
  {
    body: `Install the CLI whichever way you already have:

\`\`\`sh
brew tap souravsspace/tap && brew install nopass
npm install -g nopass-cli
cargo install nopass-cli
nix profile install github:souravsspace/nopass
\`\`\`

Then make a store. The first run asks where the private key should live and for a master passphrase, and writes the key already encrypted:

\`\`\`sh
nopass init
\`\`\`

Back the private key up somewhere safe. Without it — or without the passphrase — nobody can read your passwords, including you.`,
    id: "install",
    title: "Install and create a store",
  },
  {
    body: `The store is a directory tree, one file per entry:

\`\`\`
~/.nopass/
├── .nopass-id          recipients (public keys) for everything below
├── web/
│   ├── github.com.np   one age ciphertext per entry
│   └── gitlab.com.np
└── team/shared/
    ├── .nopass-id      different recipients, just for this subtree
    └── wifi.np
\`\`\`

Each \`.np\` file is encrypted to every public key in the nearest \`.nopass-id\` walking up the tree, so a shared folder is a folder with more recipients in it.

An entry body is one line of secret, then \`key: value\` lines. A \`type:\` line says what it holds — a login by default, or a card, an identity or a passkey:

\`\`\`
4111111111114242
type: card
cardholder: Sana Qureshi
exp-month: 04
exp-year: 2029
\`\`\`

Entry names are file names, so **names are not encrypted**. Do not put a secret in a name.`,
    id: "the-store",
    title: "How the store is laid out",
  },
  {
    body: `\`\`\`sh
nopass                          # the tree
nopass show web/github          # print an entry
nopass show -c web/github       # copy line 1, cleared after 45s
nopass show --field username web/github

nopass generate web/github 32   # random password, stored and printed
nopass insert mail/proton       # type it yourself
nopass insert --type card --field cardholder="Sana Q" --field exp=04/2029 cards/visa
nopass insert --type identity --field given-name=Sana --field city=Dhaka me/home

nopass set web/github username=someone@else   # change one field
nopass set web/github totp=                   # clear one
nopass edit web/github                        # open it in $EDITOR
nopass mv / cp / rm                           # move, copy, delete

nopass find github              # search names
nopass grep alice               # search decrypted contents
\`\`\`

\`set\` rewrites only the fields you name and leaves every other line alone, so notes and fields it does not recognise survive.`,
    id: "cli",
    title: "The command line",
  },
  {
    body: `Turn on history and sync once:

\`\`\`sh
nopass git init
nopass git remote add origin git@github.com:you/passwords.git
\`\`\`

From then on every change commits, pulls with rebase, and pushes. Offline, changes commit locally and nopass says so; push later with \`nopass git push\`. Any git subcommand works through the passthrough.

A second machine needs two things: your identity file, and a clone of the store. Same key, same store.`,
    id: "sync",
    title: "History and sync",
  },
  {
    body: `Build the extension and the host, load it unpacked, and register the host with the browser:

\`\`\`sh
bun install
cd tools/chrome && bun run build     # or tools/firefox
cargo build --release
./target/release/nopass-host install --browser chrome --extension-id <id>
\`\`\`

What it does on a page:

- **Focus a login field** and it offers the entries that match the site, by the entry's \`url:\` line or by a name ending in the host. Picking one fills the form.
- **Focus a card or address field** and it offers your cards or identities. One pick fills the whole form — number, name, expiry, security code, or the entire address.
- **Submit something new** and it asks whether to keep it, with a name you can rewrite. It stays quiet when the store already holds that login, that card, or that person.
- **The popup** lists what matches the page and everything else, creates entries of any kind, generates passwords, and edits an entry behind a confirmation.

A page's own script can never read the panels — they render in closed shadow roots — and can never cause a fill.`,
    id: "extension",
    title: "The browser extension",
  },
  {
    body: `Unlocking is deliberately slow, so nopass has an agent: a small background process that holds unlocked identities **in memory** for a configured number of seconds. Nothing about the cache touches disk.

\`\`\`sh
nopass agent status    # what is cached, and for how long
nopass agent stop      # forget it
nopass lock            # the same, from anywhere
\`\`\`

Set \`cache-ttl\` in \`~/.config/nopass/config\` to a number of seconds to turn it on; it is zero by default, which means every read asks.

The browser shares that unlock. Unlocking in the popup unlocks your terminal, and \`nopass lock\` locks the browser. Reads may use the cache; anything that changes the store asks you directly.`,
    id: "agent",
    title: "The agent and cache-ttl",
  },
  {
    body: `nopass can be the authenticator a site knows you by. It generates a P-256 keypair, keeps the private half as an ordinary encrypted entry, and signs the site's challenge.

\`\`\`sh
nopass webauthn register --rp github.com --user you@example.com keys/github
nopass webauthn list
nopass webauthn assert --challenge <base64url> keys/github
nopass webauthn verify keys/github < assertion.json
\`\`\`

\`register\` and \`assert\` print exactly the JSON a browser hands to a site, so it can be posted to a relying party unchanged. \`verify\` is the site's half, run locally, so you can watch a signature hold.

The signature counter moves on every use and is written back before the assertion is printed — a counter that goes backwards is what a site reads as a cloned credential.

**Losing the store loses passkeys.** The site keeps the other half of the key and there is no password to fall back on.`,
    id: "passkeys",
    title: "Passkeys",
  },
  {
    body: `- Entries are encrypted with **age**: X25519 key agreement, ChaCha20-Poly1305 authenticated encryption. Nothing rolled by hand.
- The **identity file** is encrypted at rest behind key slots — a passphrase, and optionally a FIDO2 security key. Any slot opens it, and slots are added and removed independently.
- **Entry names are not encrypted.** They are file names.
- **The prompt before a write is a program check**, not cryptography: encryption needs only the public key, so anything running as you could write entries anyway. It defends a borrowed terminal, not a compromised account.
- **The browser never sees your passphrase** except as the thing you type into the unlock screen, which goes straight to the host.
- **The extension can create an entry and change fields of one.** It can never move an entry, delete one, or claim a name that is taken.
- **The clipboard is not cleared** by the extension.`,
    id: "security",
    title: "What is protected, and what is not",
  },
];

/** The docs as plain markdown, for `/docs.md` and for `llms-full.txt`. */
export function asMarkdown(): string {
  const body = SECTIONS.map(
    (section) => `## ${section.title}\n\n${section.body}`
  ).join("\n\n");
  return `# nopass documentation\n\n${body}\n`;
}
