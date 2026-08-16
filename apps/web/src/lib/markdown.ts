/**
 * The little markdown the docs are written in.
 *
 * Paragraphs, fenced code, bullet lists, inline code, bold, and links —
 * that is everything `lib/docs.ts` uses, and a renderer for it is thirty
 * lines. A markdown library would be a dependency, a bundle, and a
 * configuration file for the same output.
 *
 * The docs are ours, so the input is trusted; every value that reaches the
 * page is escaped anyway, because a renderer that only escapes what it
 * expects to be dangerous is one edit away from not.
 */

const ESCAPES: Record<string, string> = {
  '"': "&quot;",
  "&": "&amp;",
  "<": "&lt;",
  ">": "&gt;",
};

function escapeHtml(text: string): string {
  return text.replace(
    /[&<>"]/g,
    (character) => ESCAPES[character] ?? character
  );
}

/** Inline: `code`, **bold**, [text](href). Escaped first, so order is safe. */
function inline(text: string): string {
  return escapeHtml(text)
    .replace(/`([^`]+)`/g, "<code>$1</code>")
    .replace(/\*\*([^*]+)\*\*/g, "<strong>$1</strong>")
    .replace(
      /\[([^\]]+)\]\(([^)]+)\)/g,
      '<a href="$2" rel="noreferrer">$1</a>'
    );
}

/** A block, and where the next one starts. */
type Block = [html: string, next: number];

function fence(lines: string[], from: number): Block {
  const code: string[] = [];
  let at = from + 1;
  while (at < lines.length && !(lines[at] ?? "").startsWith("```")) {
    code.push(lines[at] ?? "");
    at += 1;
  }
  return [`<pre><code>${escapeHtml(code.join("\n"))}</code></pre>`, at + 1];
}

function bullets(lines: string[], from: number): Block {
  const items: string[] = [];
  let at = from;
  while (at < lines.length && (lines[at] ?? "").startsWith("- ")) {
    items.push(`<li>${inline((lines[at] ?? "").slice(2))}</li>`);
    at += 1;
  }
  return [`<ul>${items.join("")}</ul>`, at];
}

function paragraph(lines: string[], from: number): Block {
  const said: string[] = [];
  let at = from;
  while (at < lines.length) {
    const line = lines[at] ?? "";
    if (line.trim() === "" || line.startsWith("```") || line.startsWith("- ")) {
      break;
    }
    said.push(line);
    at += 1;
  }
  return [`<p>${inline(said.join(" "))}</p>`, at];
}

export function marked(source: string): string {
  const lines = source.split("\n");
  const out: string[] = [];
  let at = 0;

  while (at < lines.length) {
    const line = lines[at] ?? "";
    if (line.trim() === "") {
      at += 1;
      continue;
    }

    let read = paragraph;
    if (line.startsWith("```")) {
      read = fence;
    } else if (line.startsWith("- ")) {
      read = bullets;
    }

    const [html, next] = read(lines, at);
    out.push(html);
    at = next;
  }

  return out.join("\n");
}
