#!/usr/bin/env node
// Downloads the nopass binary for this platform from the GitHub release that
// matches this package's version, checks it against the release's own sha256,
// and unpacks it next to this file. Run by npm as a postinstall script.
"use strict";

const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const crypto = require("node:crypto");
const https = require("node:https");
const { execFileSync } = require("node:child_process");

const { version } = require("./package.json");
const REPO = "souravsspace/nopass";

// Exactly the targets .github/workflows/release.yml builds.
const TARGETS = {
  "darwin x64": "x86_64-apple-darwin",
  "darwin arm64": "aarch64-apple-darwin",
  "linux x64": "x86_64-unknown-linux-musl",
  "linux arm64": "aarch64-unknown-linux-musl",
};

function target() {
  const key = `${process.platform} ${process.arch}`;
  const t = TARGETS[key];
  if (!t) {
    throw new Error(
      `nopass has no prebuilt binary for ${key}. ` +
        `Build it from source instead: cargo install nopass-cli`,
    );
  }
  return t;
}

// GitHub redirects release downloads at objects.githubusercontent.com.
function get(url, redirects = 5) {
  return new Promise((resolve, reject) => {
    https
      .get(url, { headers: { "user-agent": `nopass-cli/${version}` } }, (res) => {
        const { statusCode, headers } = res;
        if (statusCode >= 300 && statusCode < 400 && headers.location) {
          res.resume();
          if (redirects === 0) {
            reject(new Error(`too many redirects for ${url}`));
            return;
          }
          resolve(get(new URL(headers.location, url).toString(), redirects - 1));
          return;
        }
        if (statusCode !== 200) {
          res.resume();
          reject(new Error(`GET ${url} returned ${statusCode}`));
          return;
        }
        const chunks = [];
        res.on("data", (c) => chunks.push(c));
        res.on("end", () => resolve(Buffer.concat(chunks)));
        res.on("error", reject);
      })
      .on("error", reject);
  });
}

async function main() {
  const t = target();
  const name = `nopass-${version}-${t}`;
  const base = `https://github.com/${REPO}/releases/download/v${version}/${name}.tar.gz`;

  const [archive, checksum] = await Promise.all([get(base), get(`${base}.sha256`)]);

  // The release writes "<sha256>  <file>.tar.gz".
  const want = checksum.toString("utf8").trim().split(/\s+/)[0];
  const got = crypto.createHash("sha256").update(archive).digest("hex");
  if (want !== got) {
    throw new Error(`checksum mismatch for ${name}.tar.gz: expected ${want}, got ${got}`);
  }

  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "nopass-"));
  try {
    const tarball = path.join(tmp, "nopass.tar.gz");
    fs.writeFileSync(tarball, archive);
    execFileSync("tar", ["-xzf", tarball, "-C", tmp]);

    const dest = path.join(__dirname, "binary");
    fs.mkdirSync(dest, { recursive: true });
    const binary = path.join(dest, "nopass");
    fs.copyFileSync(path.join(tmp, name, "nopass"), binary);
    fs.chmodSync(binary, 0o755);
  } finally {
    fs.rmSync(tmp, { recursive: true, force: true });
  }
}

main().catch((err) => {
  console.error(`nopass: ${err.message}`);
  process.exit(1);
});
