import type { Locator, Page } from "@playwright/test";
import { expect, test } from "@playwright/test";

const popup = (page: Page): Locator => page.getByTestId("popup-surface");

const FILL_GITHUB = /fill\(web\/github\.com\)/;
const ANY_REVEAL = /reveal\(/;
const REVEAL_GITHUB = /reveal\(web\/github\.com\)/;
const LOCK_CALL = /lock\(\)/;
const HOST_INSTALL = /nopass-host install/;
const DARK = /dark/;

async function scenario(page: Page, label: string) {
  await page.goto("/");
  await page.getByRole("button", { exact: true, name: label }).click();
}

test.describe("the popup", () => {
  test("asks for a passphrase when the store is locked", async ({ page }) => {
    await scenario(page, "Locked");

    await expect(popup(page).getByText("Locked")).toBeVisible();
    await expect(popup(page).getByLabel("Passphrase")).toBeVisible();
    await expect(
      popup(page).getByRole("button", { name: "Unlock" })
    ).toBeDisabled();
  });

  test("unlocks and then lists what matches the page", async ({ page }) => {
    await scenario(page, "Locked");

    await popup(page)
      .getByLabel("Passphrase")
      .fill("correct horse battery staple");
    await popup(page).getByRole("button", { name: "Unlock" }).click();

    await expect(popup(page).getByText("Unlocked")).toBeVisible();
    await expect(popup(page).getByPlaceholder("Search entries")).toBeVisible();
    await expect(popup(page).getByText("sana@example.com")).toBeVisible();
  });

  test("never puts a passphrase on screen in clear text", async ({ page }) => {
    await scenario(page, "Locked");

    const field = popup(page).getByLabel("Passphrase");
    await field.fill("hunter2");
    await expect(field).toHaveAttribute("type", "password");
  });

  test("narrows the list as you type", async ({ page }) => {
    await scenario(page, "Unlocked");
    const rows = popup(page).getByRole("listitem");
    await expect(rows).toHaveCount(2);

    await popup(page).getByPlaceholder("Search entries").fill("work");
    await expect(rows).toHaveCount(1);

    await popup(page).getByPlaceholder("Search entries").fill("zzzz");
    await expect(rows).toHaveCount(0);
  });

  test("fills through the bridge when a row is chosen", async ({ page }) => {
    await scenario(page, "Unlocked");
    await popup(page).getByText("sana@example.com").click();

    await expect(page.getByText(FILL_GITHUB)).toBeVisible();
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
    await popup(page).getByRole("button", { name: "Lock" }).click();

    await expect(page.getByText(LOCK_CALL)).toBeVisible();
    await expect(popup(page).getByLabel("Passphrase")).toBeVisible();
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
