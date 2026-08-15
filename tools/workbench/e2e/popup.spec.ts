import type { Locator, Page } from "@playwright/test";
import { expect, test } from "@playwright/test";

const popup = (page: Page): Locator => page.getByTestId("popup-surface");

const FILL_GITHUB = /fill\(web\/github\.com\)/;
const FILL_GITHUB_WORK = /fill\(web\/github\.com-work\)/;
const ANY_REVEAL = /reveal\(/;
const REVEAL_GITHUB = /reveal\(web\/github\.com\)/;
const SAVE_EXAMPLE = /save\(web\/example\.com, login\)/;
const GENERATE_CALL = /generate\(24, symbols: true\)/;
const SAVE_CARD = /save\(cards\/amex, card\)/;
const SAVE_IDENTITY = /save\(me\/work, identity\)/;
const UPDATE_GITHUB = /update\(web\/github\.com\)/;
const VIEW_GITHUB = /^View web\/github\.com$/;
const REPLACES_WHAT_IS_STORED = /This replaces what is stored/;
const TWENTY_FOUR_CHARACTERS = /^.{24}$/;
const GITHUB_PASSWORD = "9x!Kd2pQvr4TmZ";
const ITEMS_CALL = /items\(all\)/;
const LOCK_CALL = /lock\(\)/;
const HOST_INSTALL = /nopass-host install/;
const TTL_CLOCK = /4:2\d/;
const REFUSED = "that passphrase did not open the store";
const DARK = /dark/;

async function scenario(page: Page, label: string) {
  await page.goto("/");
  await page.getByRole("button", { exact: true, name: label }).click();
}

test.describe("the popup", () => {
  test("asks for a passphrase when the store is locked", async ({ page }) => {
    await scenario(page, "Locked");

    await expect(
      popup(page).getByText("Locked", { exact: true })
    ).toBeVisible();
    await expect(popup(page).getByText("Your store is locked")).toBeVisible();
    await expect(popup(page).getByLabel("Master passphrase")).toBeVisible();
    await expect(
      popup(page).getByRole("button", { name: "Unlock" })
    ).toBeDisabled();
  });

  test("unlocks and then lists what matches the page", async ({ page }) => {
    await scenario(page, "Locked");

    await popup(page)
      .getByLabel("Master passphrase")
      .fill("correct horse battery staple");
    await popup(page).getByRole("button", { name: "Unlock" }).click();

    await expect(
      popup(page).getByRole("button", { name: "Lock now" })
    ).toBeVisible();
    await expect(
      popup(page).getByPlaceholder("Search your store")
    ).toBeVisible();
    await expect(popup(page).getByText("sana@example.com")).toBeVisible();
  });

  test("says why the host refused, and stays locked", async ({ page }) => {
    await scenario(page, "Locked");

    await popup(page).getByLabel("Master passphrase").fill("not the one");
    await popup(page).getByRole("button", { name: "Unlock" }).click();

    await expect(popup(page).getByRole("alert")).toHaveText(REFUSED);
    await expect(popup(page).getByText("Your store is locked")).toBeVisible();

    // The complaint is about what was typed, so it goes when that changes.
    await popup(page).getByLabel("Master passphrase").fill("n");
    await expect(popup(page).getByRole("alert")).toHaveCount(0);
  });

  test("counts the lease down rather than inventing one", async ({ page }) => {
    await scenario(page, "Unlocked");

    // The mock hands back 268 seconds; the pill renders the host's number.
    await expect(
      popup(page).getByRole("button", { name: "Lock now" })
    ).toContainText(TTL_CLOCK);
  });

  test("never puts a passphrase on screen in clear text", async ({ page }) => {
    await scenario(page, "Locked");

    const field = popup(page).getByLabel("Master passphrase");
    await field.fill("hunter2");
    await expect(field).toHaveAttribute("type", "password");
  });

  test("separates this page's entries from the rest of the store", async ({
    page,
  }) => {
    await scenario(page, "Unlocked");

    await expect(popup(page).getByText("This page")).toBeVisible();
    await expect(popup(page).getByText("All items")).toBeVisible();
    // `items` is names, kinds and hints — never a secret — and is the only
    // reason the second section exists.
    await expect(page.getByText(ITEMS_CALL)).toBeVisible();
  });

  test("narrows the list as you type", async ({ page }) => {
    await scenario(page, "Unlocked");
    // Four logins, a card and an identity.
    const rows = popup(page).getByRole("listitem");
    await expect(rows).toHaveCount(6);

    await popup(page).getByPlaceholder("Search your store").fill("work");
    await expect(rows).toHaveCount(1);

    await popup(page).getByPlaceholder("Search your store").fill("zzzz");
    await expect(rows).toHaveCount(0);
  });

  test("fills through the bridge when a row is chosen", async ({ page }) => {
    await scenario(page, "Unlocked");
    await popup(page).getByText("sana@example.com").click();

    await expect(page.getByText(FILL_GITHUB)).toBeVisible();
  });

  test("moves with the arrow keys and fills on Enter", async ({ page }) => {
    await scenario(page, "Unlocked");

    const search = popup(page).getByPlaceholder("Search your store");
    await search.press("ArrowDown");
    await search.press("Enter");

    await expect(page.getByText(FILL_GITHUB_WORK)).toBeVisible();
  });

  test("reveals a password only when asked to copy one", async ({ page }) => {
    await scenario(page, "Unlocked");
    await expect(page.getByText(ANY_REVEAL)).toHaveCount(0);

    await popup(page)
      .getByRole("button", {
        exact: true,
        name: "Copy the password for web/github.com",
      })
      .click();

    await expect(page.getByText(REVEAL_GITHUB)).toBeVisible();
  });

  test("opens one entry on its own screen without leaving the list behind", async ({
    page,
  }) => {
    await scenario(page, "Unlocked");
    await popup(page)
      .getByRole("button", { exact: true, name: "View web/github.com" })
      .click();

    // Everything the entry holds, and the password masked until asked for.
    await expect(popup(page).getByText("sana@example.com")).toBeVisible();
    await expect(popup(page).getByText("https://github.com")).toBeVisible();
    await expect(popup(page).getByText(GITHUB_PASSWORD)).toHaveCount(0);

    await popup(page)
      .getByRole("button", { name: "Show the Password" })
      .click();
    await expect(popup(page).getByText(GITHUB_PASSWORD)).toBeVisible();

    await popup(page).getByRole("button", { name: "Back" }).click();
    await expect(
      popup(page).getByPlaceholder("Search your store")
    ).toBeVisible();
  });

  test("saves a new login through the bridge", async ({ page }) => {
    await scenario(page, "Unlocked");
    await popup(page).getByRole("button", { name: "New login" }).click();

    // Pre-filled from the tab it was opened over, so the common case is a
    // password and a Save.
    await expect(popup(page).getByLabel("Name", { exact: true })).toHaveValue(
      "web/github.com"
    );
    await expect(popup(page).getByLabel("Website")).toHaveValue(
      "https://github.com"
    );

    await popup(page)
      .getByLabel("Name", { exact: true })
      .fill("web/example.com");
    await popup(page)
      .getByLabel("Username", { exact: true })
      .fill("sana@example.com");
    await popup(page).getByLabel("Password", { exact: true }).fill("hunter2");
    await popup(page).getByRole("button", { name: "Save to store" }).click();

    await expect(page.getByText(SAVE_EXAMPLE)).toBeVisible();
    await expect(popup(page).getByText("example.com added.")).toBeVisible();
  });

  test("generates a password rather than asking for one", async ({ page }) => {
    await scenario(page, "Unlocked");
    await popup(page).getByRole("button", { name: "New login" }).click();

    await popup(page).getByLabel("Length").fill("24");
    await popup(page).getByRole("button", { name: "Generate" }).click();

    await expect(page.getByText(GENERATE_CALL)).toBeVisible();
    // Shown, not masked: this is the one moment the password is worth reading.
    const field = popup(page).getByLabel("Password", { exact: true });
    await expect(field).toHaveAttribute("type", "text");
    await expect(field).toHaveValue(TWENTY_FOUR_CHARACTERS);
  });

  test("refuses to overwrite a name that is already taken", async ({
    page,
  }) => {
    await scenario(page, "Unlocked");
    await popup(page).getByRole("button", { name: "New login" }).click();

    await popup(page).getByLabel("Password", { exact: true }).fill("owned");
    await popup(page).getByRole("button", { name: "Save to store" }).click();

    await expect(popup(page).getByRole("alert")).toContainText(
      "already in the store"
    );
    await expect(
      popup(page).getByLabel("Name", { exact: true })
    ).toHaveAttribute("aria-invalid", "true");
  });

  test("locks again on request", async ({ page }) => {
    await scenario(page, "Unlocked");
    await popup(page).getByRole("button", { name: "Lock now" }).click();

    await expect(page.getByText(LOCK_CALL)).toBeVisible();
    await expect(popup(page).getByLabel("Master passphrase")).toBeVisible();
  });

  test("says what to run when there is no store", async ({ page }) => {
    await scenario(page, "No store");

    await expect(popup(page).getByText("No store yet")).toBeVisible();
    await expect(popup(page).getByText("nopass init")).toBeVisible();
  });

  test("says how to register the host when it is missing", async ({ page }) => {
    await scenario(page, "Host missing");

    await expect(
      popup(page).getByText("The nopass host is not reachable")
    ).toBeVisible();
    await expect(popup(page).getByText(HOST_INSTALL)).toBeVisible();
  });

  test("explains an empty result rather than showing a blank panel", async ({
    page,
  }) => {
    await scenario(page, "No matches");

    await expect(
      popup(page).getByText("Nothing stored for this site.")
    ).toBeVisible();
  });

  test("holds its shape in dark mode", async ({ page }) => {
    await scenario(page, "Unlocked");
    await page.getByRole("button", { name: "Dark" }).click();

    await expect(page.locator("html")).toHaveClass(DARK);
    await expect(popup(page).getByText("sana@example.com")).toBeVisible();
  });
});

test.describe("the inline dropdown", () => {
  test("renders its rows inside a shadow root", async ({ page }) => {
    await page.goto("/");
    const rows = page.getByTestId("dropdown-surface").locator(".np-row");

    await expect(rows).toHaveCount(3);
    await expect(rows.first()).toContainText("sana@example.com");
  });

  test("carries no password into the page", async ({ page }) => {
    await page.goto("/");

    const text = await page.getByTestId("dropdown-surface").innerText();
    expect(text).not.toContain("9x!Kd2pQvr4TmZ");
  });
});

test.describe("cards and identities", () => {
  test("saves a card, and never shows its number in the list", async ({
    page,
  }) => {
    await scenario(page, "Unlocked");
    await popup(page).getByRole("button", { name: "New login" }).click();
    await popup(page).getByRole("tab", { name: "Card" }).click();

    await popup(page).getByLabel("Name", { exact: true }).fill("cards/amex");
    await popup(page)
      .getByLabel("Card number", { exact: true })
      .fill("378282246310005");
    await popup(page).getByLabel("Cardholder").fill("Sana Qureshi");
    await popup(page).getByLabel("Expiry month").fill("04");
    await popup(page).getByLabel("Expiry year").fill("2029");
    await popup(page).getByRole("button", { name: "Save to store" }).click();

    await expect(page.getByText(SAVE_CARD)).toBeVisible();
    await expect(popup(page).getByText("amex added.")).toBeVisible();

    // The row reads as its tail. The number itself is on the entry screen,
    // behind Show, and nowhere else.
    await expect(popup(page).getByText("•••• 0005")).toBeVisible();
    await expect(popup(page).getByText("378282246310005")).toBeHidden();
  });

  test("saves an identity, which has no secret to type", async ({ page }) => {
    await scenario(page, "Unlocked");
    await popup(page).getByRole("button", { name: "New login" }).click();
    await popup(page).getByRole("tab", { name: "Identity" }).click();

    await expect(
      popup(page).getByLabel("Card number", { exact: true })
    ).toBeHidden();

    await popup(page).getByLabel("Name", { exact: true }).fill("me/work");
    await popup(page).getByLabel("First name").fill("Sana");
    await popup(page).getByLabel("Last name").fill("Qureshi");
    await popup(page).getByLabel("Country").fill("Bangladesh");
    await popup(page).getByRole("button", { name: "Save to store" }).click();

    await expect(page.getByText(SAVE_IDENTITY)).toBeVisible();
    await expect(popup(page).getByText("work added.")).toBeVisible();
  });

  test("edits an entry, and asks before replacing it", async ({ page }) => {
    await scenario(page, "Unlocked");
    await popup(page).getByRole("button", { name: VIEW_GITHUB }).click();
    await popup(page).getByRole("button", { name: "Edit" }).click();

    await popup(page).getByLabel("Username").fill("someone@else");
    await popup(page).getByRole("button", { name: "Save changes" }).click();

    // The first press asks; nothing has been written yet.
    await expect(popup(page).getByText(REPLACES_WHAT_IS_STORED)).toBeVisible();
    await expect(page.getByText(UPDATE_GITHUB)).toBeHidden();

    await popup(page).getByRole("button", { name: "Replace" }).click();
    await expect(page.getByText(UPDATE_GITHUB)).toBeVisible();
    await expect(popup(page).getByText("github.com updated.")).toBeVisible();
  });
});
