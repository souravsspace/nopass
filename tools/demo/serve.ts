/**
 * The demo pages, over http.
 *
 * A `file://` page gets no content script — the extension matches `http` and
 * `https` — so trying the extension by hand needs a server, and so do the
 * browser tests. This is the smallest one that will do: static files, one
 * port, no dependencies.
 */

const ROOT = new URL("./public/", import.meta.url);
const PORT = Number(process.env.PORT ?? 8790);

const server = Bun.serve({
  port: PORT,
  fetch(request) {
    const { pathname } = new URL(request.url);
    const name = pathname === "/" ? "index.html" : pathname.slice(1);
    const file = Bun.file(new URL(name, ROOT));
    return file.exists().then((there) =>
      there ? new Response(file) : new Response("not found", { status: 404 })
    );
  },
});

process.stdout.write(`demo pages on http://127.0.0.1:${server.port}\n`);
