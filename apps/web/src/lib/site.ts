/**
 * Where this site lives.
 *
 * One constant, because a canonical URL that disagrees with the sitemap, the
 * Open Graph tags or `llms.txt` is worse than no canonical URL at all. Point
 * a domain here and everything follows; nothing else in the site hard-codes
 * a host.
 */
export const SITE = {
  description:
    "A password manager whose store is a folder of encrypted files you own. Fills logins, cards and addresses in the browser, holds passkeys, and syncs over your own git remote.",
  name: "nopass",
  repository: "https://github.com/souravsspace/nopass",
  tagline: "Your passwords, in a folder you own.",
  /** Change this one line to move the site to a domain. */
  url: "https://nopass.dev",
} as const;

export function canonical(path: string): string {
  return new URL(path, SITE.url).href;
}
