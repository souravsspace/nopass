/**
 * The popup takes its light or dark from the system.
 *
 * An extension page has no site to inherit from and no settings screen worth
 * spending a row of a 360px popup on, so the browser's own colour preference
 * is the only signal there is — the same one the injected dropdown already
 * reads through `prefers-color-scheme` in `dropdown.ts`.
 *
 * The stylesheet keys dark off a `.dark` class rather than that media query,
 * because the `dark:` utilities in the shared components only fire on the
 * class. So the query is mirrored onto the element here instead.
 */

const DARK = "(prefers-color-scheme: dark)";

/**
 * Match the document to the system, and keep matching it for as long as the
 * popup is open. The listener dies with the page, which is why nothing is
 * handed back to unsubscribe with.
 */
export function followSystemTheme() {
  const system = window.matchMedia(DARK);
  const apply = () => {
    document.documentElement.classList.toggle("dark", system.matches);
  };
  apply();
  system.addEventListener("change", apply);
}
