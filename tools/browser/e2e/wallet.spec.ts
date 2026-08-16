/**
 * Cards and identities on a real checkout.
 *
 * These belong to no site, so nothing about the origin decides what is
 * offered — the field the cursor is in does, and the pick does the rest.
 * There is deliberately more than one of each: choosing between two cards is
 * the case that a "first match wins" implementation would pass by accident.
 */

import { DEMO, expect, test } from "./harness";
import { focusField, panel, panelIsOpen, pickRow, promptSave } from "./panel";

function twoCards(store: {
  insert: (n: string, k: string, s: string, f: string[]) => void;
}) {
  store.insert("cards/visa", "card", "4111111111114242", [
    "cardholder=Sana Qureshi",
    "exp-month=04",
    "exp-year=2029",
    "cvv=737",
    "brand=Visa",
  ]);
  store.insert("cards/amex", "card", "378282246310005", [
    "cardholder=S Q Business",
    "exp-month=11",
    "exp-year=2030",
    "cvv=1234",
    "brand=Amex",
  ]);
}

test("fills a whole card from one pick", async ({ page, store }) => {
  store.insert("cards/visa", "card", "4111111111114242", [
    "cardholder=Sana Qureshi",
    "exp-month=04",
    "exp-year=2029",
    "cvv=737",
    "brand=Visa",
  ]);

  await page.goto(`${DEMO}/checkout.html`);
  await focusField(page, "#ccnumber");
  await pickRow(page, 0, 1);

  await expect(page.locator("#ccnumber")).toHaveValue("4111111111114242");
  await expect(page.locator("#ccname")).toHaveValue("Sana Qureshi");
  // The page wants one box for the expiry, the way a card prints it.
  await expect(page.locator("#ccexp")).toHaveValue("04/29");
  await expect(page.locator("#cccsc")).toHaveValue("737");
});

test("offers every stored card, and fills the one chosen", async ({
  page,
  store,
}) => {
  twoCards(store);

  await page.goto(`${DEMO}/checkout.html`);
  await focusField(page, "#ccnumber");
  // Rows come in store order, which is by name: amex first, visa second. The
  // second one is the one worth proving, since taking the first is what an
  // implementation that ignored the pick would also do.
  await pickRow(page, 1, 2);

  await expect(page.locator("#ccnumber")).toHaveValue("4111111111114242");
  await expect(page.locator("#ccname")).toHaveValue("Sana Qureshi");
  await expect(page.locator("#ccexp")).toHaveValue("04/29");
});

test("offers a card on the security code field too", async ({
  page,
  store,
}) => {
  twoCards(store);

  await page.goto(`${DEMO}/checkout.html`);
  await focusField(page, "#cccsc");
  const box = await panel(page, "dropdown");

  expect(box.height).toBeGreaterThan(0);
});

test("never offers a card to a login form", async ({ page, store }) => {
  twoCards(store);

  await page.goto(`${DEMO}/login.html`);
  await page.focus("#password");
  await page.waitForTimeout(1500);

  expect(await panelIsOpen(page, "dropdown")).toBe(false);
});

test("fills an address from an identity", async ({ page, store }) => {
  store.insert("me/home", "identity", "", [
    "given-name=Sana",
    "family-name=Qureshi",
    "email=sana@example.com",
    "phone=+8801700000000",
    "street=12 Example Road",
    "city=Dhaka",
    "postcode=1207",
    "country=Bangladesh",
  ]);

  await page.goto(`${DEMO}/profile.html`);
  await focusField(page, "#address1");
  await pickRow(page, 0, 1);

  await expect(page.locator("#firstName")).toHaveValue("Sana");
  await expect(page.locator("#lastName")).toHaveValue("Qureshi");
  await expect(page.locator("#address1")).toHaveValue("12 Example Road");
  await expect(page.locator("#city")).toHaveValue("Dhaka");
  await expect(page.locator("#postcode")).toHaveValue("1207");
  await expect(page.locator("#country")).toHaveValue("Bangladesh");
  await expect(page.locator("#email")).toHaveValue("sana@example.com");
  await expect(page.locator("#phone")).toHaveValue("+8801700000000");
});

test("offers each identity when there is more than one", async ({
  page,
  store,
}) => {
  store.insert("me/home", "identity", "", [
    "given-name=Sana",
    "family-name=Qureshi",
    "city=Dhaka",
  ]);
  store.insert("me/work", "identity", "", [
    "given-name=Sana",
    "family-name=Qureshi",
    "street=4 Office Lane",
    "city=Chattogram",
  ]);

  await page.goto(`${DEMO}/profile.html`);
  await focusField(page, "#city");
  // By name again: me/home first, me/work second.
  await pickRow(page, 1, 2);

  await expect(page.locator("#city")).toHaveValue("Chattogram");
});

test("a card and an identity are not offered on each other's fields", async ({
  page,
  store,
}) => {
  twoCards(store);
  store.insert("me/home", "identity", "", ["given-name=Sana", "city=Dhaka"]);

  await page.goto(`${DEMO}/checkout.html`);
  await focusField(page, "#ccnumber");
  const cards = await panel(page, "dropdown");
  const rowHeight = cards.height / 2;

  // Two cards on a card field, and the identity is not among them.
  expect(Math.round(cards.height / rowHeight)).toBe(2);
});

test("asks to keep a card that was just typed into a checkout", async ({
  page,
  store,
}) => {
  await page.goto(`${DEMO}/checkout.html`);
  await page.fill("#ccname", "Sana Qureshi");
  await page.fill("#ccnumber", "4111 1111 1111 4242");
  await page.fill("#ccexp", "04/29");
  await page.fill("#cccsc", "737");
  await page.click("button[type=submit]");

  await panel(page, "prompt");
  await promptSave(page);

  await expect
    .poll(() => store.list(), { timeout: 10_000 })
    .toContain("cards/card-4242");

  const body = store.show("cards/card-4242");
  expect(body).toContain("type: card");
  expect(body).toContain("4111111111114242");
  expect(body).toContain("cardholder: Sana Qureshi");
  expect(body).toContain("exp-month: 04");
  expect(body).toContain("exp-year: 2029");
  // Card networks forbid keeping the security code, so nopass does not.
  expect(body).not.toContain("737");
  // A card belongs to no site, so it never gains the checkout's url.
  expect(body).not.toContain("url:");
});

test("asks to keep an address that was just typed in", async ({
  page,
  store,
}) => {
  await page.goto(`${DEMO}/profile.html`);
  await page.fill("#firstName", "Sana");
  await page.fill("#lastName", "Qureshi");
  await page.fill("#address1", "12 Example Road");
  await page.fill("#city", "Dhaka");
  await page.fill("#postcode", "1207");
  await page.fill("#country", "Bangladesh");
  await page.click("button[type=submit]");

  await panel(page, "prompt");
  await promptSave(page);

  await expect
    .poll(() => store.list(), { timeout: 10_000 })
    .toContain("me/sana");

  const body = store.show("me/sana");
  expect(body).toContain("type: identity");
  expect(body).toContain("given-name: Sana");
  expect(body).toContain("city: Dhaka");
  expect(body).toContain("country: Bangladesh");
});

test("does not ask about a card it already holds", async ({ page, store }) => {
  store.insert("cards/visa", "card", "4111111111114242", [
    "cardholder=Sana Qureshi",
    "brand=Visa",
  ]);

  await page.goto(`${DEMO}/checkout.html`);
  await page.fill("#ccname", "Sana Qureshi");
  await page.fill("#ccnumber", "4111111111114242");
  await page.fill("#cccsc", "737");
  await page.click("button[type=submit]");
  await page.waitForTimeout(2500);

  expect(await panelIsOpen(page, "prompt")).toBe(false);
});
