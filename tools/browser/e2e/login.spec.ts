/**
 * A login, end to end: offered on the page, filled into it, and saved back.
 *
 * Every assertion that matters here is made against the store on disk rather
 * than against the extension's own UI — `nopass show` reads what was actually
 * encrypted, which is the only proof that a save happened.
 */

import { DEMO, expect, test } from "./harness";
import {
  focusField,
  panel,
  panelIsOpen,
  pickRow,
  promptDismiss,
  promptSave,
  typeName,
} from "./panel";

/** Where the demo forms land after a submit. */
const DONE_PAGE = /done\.html/;

test("offers a stored login on the site it belongs to, and fills it", async ({
  page,
  store,
}) => {
  store.insert("web/127.0.0.1", "login", "stored-pw", [
    "username=sana@example.com",
  ]);

  await page.goto(`${DEMO}/login.html`);
  await focusField(page, "#password");
  await pickRow(page, 0, 1);

  await expect(page.locator("#email")).toHaveValue("sana@example.com");
  await expect(page.locator("#password")).toHaveValue("stored-pw");
});

test("offers nothing for a site with no entry", async ({ page, store }) => {
  store.insert("web/elsewhere.example", "login", "not-yours", []);

  await page.goto(`${DEMO}/login.html`);
  await page.focus("#password");
  await page.waitForTimeout(1500);

  expect(await panelIsOpen(page, "dropdown")).toBe(false);
});

test("asks to save a login it has never seen, and writes it", async ({
  page,
  store,
}) => {
  await page.goto(`${DEMO}/login.html`);
  await page.fill("#email", "new@example.com");
  await page.fill("#password", "brand-new-pw");
  await page.click("button[type=submit]");

  // The form navigates, so the prompt arrives on the page that replaced it.
  await expect(page).toHaveURL(DONE_PAGE);
  await panel(page, "prompt");
  await promptSave(page);

  await expect
    .poll(() => store.list(), { timeout: 10_000 })
    .toContain("web/127.0.0.1:8790");

  const body = store.show("web/127.0.0.1:8790");
  expect(body).toContain("brand-new-pw");
  expect(body).toContain("username: new@example.com");
  expect(body).toContain("url: http://127.0.0.1:8790");
});

test("saves under whatever name is typed into the prompt", async ({
  page,
  store,
}) => {
  await page.goto(`${DEMO}/login.html`);
  await page.fill("#email", "sana@example.com");
  await page.fill("#password", "another-pw");
  await page.click("button[type=submit]");

  await panel(page, "prompt");
  await typeName(page, "work/demo-site");
  await promptSave(page);

  await expect
    .poll(() => store.list(), { timeout: 10_000 })
    .toContain("work/demo-site");
});

test("writes nothing when the prompt is dismissed", async ({ page, store }) => {
  await page.goto(`${DEMO}/login.html`);
  await page.fill("#email", "sana@example.com");
  await page.fill("#password", "never-saved");
  await page.click("button[type=submit]");

  await panel(page, "prompt");
  await promptDismiss(page);
  await page.waitForTimeout(1000);

  expect(store.list()).toEqual([]);
});

test("does not ask about a login it already holds", async ({ page, store }) => {
  store.insert("web/127.0.0.1:8790", "login", "stored-pw", [
    "username=sana@example.com",
    "url=http://127.0.0.1:8790",
  ]);

  await page.goto(`${DEMO}/login.html`);
  await page.fill("#email", "sana@example.com");
  await page.fill("#password", "stored-pw");
  await page.click("button[type=submit]");
  await page.waitForTimeout(2000);

  expect(await panelIsOpen(page, "prompt")).toBe(false);
});

test("offers the account field of a two-step sign-in", async ({
  page,
  store,
}) => {
  store.insert("web/127.0.0.1", "login", "stored-pw", [
    "username=sana@example.com",
  ]);

  await page.goto(`${DEMO}/two-step.html`);
  await focusField(page, "#identifier");
  await pickRow(page, 0, 1);

  await expect(page.locator("#identifier")).toHaveValue("sana@example.com");
});
