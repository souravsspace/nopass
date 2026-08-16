/**
 * Reaching the extension's in-page panels from a test.
 *
 * Both panels render into **closed** shadow roots, which is a property worth
 * keeping: a page cannot read the dropdown, restyle it, or click it. That
 * applies to a test's page context too, so nothing here queries inside them.
 * What a test can see is the host element the panel is mounted on — its
 * position, its size — and what it can do is move the real mouse over it.
 *
 * Positions are derived from the panel's own box rather than hard-coded
 * pixels, so a change to padding does not break every test. The fractions
 * below track the layouts in `packages/extension/lib/dropdown.ts` and
 * `lib/prompt.ts`; if a panel's shape changes, this is the one file to fix.
 */

import { expect, type Page } from "@playwright/test";

/** A panel's host element, found by how the content script positions it. */
type Panel = "dropdown" | "prompt";

const POSITION: Record<Panel, string> = {
  dropdown: "absolute",
  prompt: "fixed",
};

export interface Box {
  height: number;
  width: number;
  x: number;
  y: number;
}

async function boxOf(page: Page, panel: Panel): Promise<Box | null> {
  return await page.evaluate((position) => {
    const host = [...document.body.children].find(
      (node) =>
        node.tagName === "DIV" &&
        getComputedStyle(node).position === position &&
        getComputedStyle(node).display !== "none"
    );
    if (!host) {
      return null;
    }
    const rect = host.getBoundingClientRect();
    return {
      height: rect.height,
      width: rect.width,
      x: rect.x,
      y: rect.y,
    };
  }, POSITION[panel]);
}

/** Wait for a panel to be on screen, and answer with where it is. */
export async function panel(page: Page, which: Panel): Promise<Box> {
  let found: Box | null = null;
  await expect
    .poll(
      async () => {
        found = await boxOf(page, which);
        return found !== null && found.height > 0;
      },
      { message: `the ${which} never appeared`, timeout: 10_000 }
    )
    .toBe(true);

  if (!found) {
    throw new Error(`the ${which} never appeared`);
  }
  return found;
}

/** Whether a panel is on screen right now, without waiting for one. */
export async function panelIsOpen(page: Page, which: Panel): Promise<boolean> {
  const box = await boxOf(page, which);
  return box !== null && box.height > 0;
}

/**
 * Put the cursor in a field and wait for the panel it should raise.
 *
 * The content script is injected at `document_idle`, so a test that focuses
 * a field the instant a page settles can beat it there — a person cannot,
 * but `page.focus` can. Focus is given up and taken again rather than waited
 * out, because a focus that arrived too early raises nothing and no later
 * event will.
 */
export async function focusField(page: Page, selector: string): Promise<void> {
  for (let attempt = 0; attempt < 4; attempt++) {
    await page.focus(selector);
    for (let tick = 0; tick < 10; tick++) {
      if (await panelIsOpen(page, "dropdown")) {
        return;
      }
      await page.waitForTimeout(100);
    }
    await page.locator(selector).blur();
    await page.waitForTimeout(200);
  }
  throw new Error(`no dropdown appeared under ${selector}`);
}

/**
 * Click one row of the dropdown.
 *
 * Rows are uniform, so the row height is the panel's own height divided by
 * how many rows the test put there — self-calibrating, and it fails loudly if
 * the panel is not showing what the test expected.
 */
export async function pickRow(
  page: Page,
  index: number,
  rows: number
): Promise<void> {
  const box = await panel(page, "dropdown");
  const rowHeight = box.height / rows;
  await page.mouse.click(box.x + 24, box.y + rowHeight * (index + 0.5));
}

/** How many rows the dropdown is showing, by its height. */
export async function rowCount(page: Page, rowHeight: number): Promise<number> {
  const box = await panel(page, "dropdown");
  return Math.round(box.height / rowHeight);
}

/*
 * The save prompt, top to bottom: title, the login it is about, a label, the
 * name box, a refusal that is usually hidden, then the two buttons. These are
 * where those sit as a fraction of the panel, measured from its own box.
 */
const NAME_BOX_FROM_TOP = 0.62;
const BUTTONS_FROM_BOTTOM = 26;
const SAVE_FROM_RIGHT = 34;
const DISMISS_FROM_RIGHT = 100;

/** Rewrite the name the prompt is offering. */
export async function typeName(page: Page, name: string): Promise<void> {
  const box = await panel(page, "prompt");
  await page.mouse.click(
    box.x + box.width / 2,
    box.y + box.height * NAME_BOX_FROM_TOP
  );
  await page.keyboard.press("ControlOrMeta+a");
  await page.keyboard.type(name);
}

export async function promptSave(page: Page): Promise<void> {
  const box = await panel(page, "prompt");
  await page.mouse.click(
    box.x + box.width - SAVE_FROM_RIGHT,
    box.y + box.height - BUTTONS_FROM_BOTTOM
  );
}

export async function promptDismiss(page: Page): Promise<void> {
  const box = await panel(page, "prompt");
  await page.mouse.click(
    box.x + box.width - DISMISS_FROM_RIGHT,
    box.y + box.height - BUTTONS_FROM_BOTTOM
  );
}
