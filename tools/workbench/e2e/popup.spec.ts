import type { Locator, Page } from "@playwright/test";
import { expect, test } from "@playwright/test";

const popup = (page: Page): Locator => page.getByTestId("popup-surface");

const FILL_GITHUB = /fill\(web\/github\.com\)/;
const FILL_GITHUB_WORK = /fill\(web\/github\.com-work\)/;
const ANY_REVEAL = /reveal\(/;
const REVEAL_GITHUB = /reveal\(web\/github\.com\)/;
const LIST_CALL = /list\(\)/;
const LOCK_CALL = /lock\(\)/;
const HOST_INSTALL = /nopass-host install/;
const TTL_CLOCK = /4:2\d/;
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
    // `list` is names only, and is the only reason the second section exists.
    await expect(page.getByText(LIST_CALL)).toBeVisible();
  });

  test("narrows the list as you type", async ({ page }) => {
    await scenario(page, "Unlocked");
    const rows = popup(page).getByRole("listitem");
    await expect(rows).toHaveCount(4);

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
