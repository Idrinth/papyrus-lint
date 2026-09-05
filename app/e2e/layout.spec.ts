import { expect, test } from "@playwright/test";

// These run against a real Chromium layout engine (see playwright.config.ts)
// specifically to catch element-size regressions the jsdom-based Vitest
// suite structurally cannot see - jsdom never computes an actual box model,
// so a collapsed drop zone, a mis-hidden tab panel, or an overlay that no
// longer matches its underlying element's dimensions would all pass there
// unnoticed.

test("renders the main layout with real, non-zero dimensions", async ({ page }) => {
  await page.goto("/");

  const container = page.locator(".container");
  const tabs = page.locator(".tabs");
  const dropZone = page.locator("#drop-zone");

  const containerBox = await container.boundingBox();
  const tabsBox = await tabs.boundingBox();
  const dropZoneBox = await dropZone.boundingBox();

  expect(containerBox?.width).toBeGreaterThan(0);
  expect(containerBox?.height).toBeGreaterThan(0);
  expect(tabsBox?.width).toBeGreaterThan(0);

  // The drop zone is meant to span the full width of the tabs panel it sits
  // in and be tall enough to be an obvious drop target, not a sliver.
  expect(dropZoneBox?.width).toBeGreaterThan(0);
  expect(dropZoneBox?.height).toBeGreaterThan(40);
  expect(dropZoneBox!.width).toBeCloseTo(tabsBox!.width, 0);
});

test("switching tabs shows exactly one panel with a real box, hides the rest", async ({ page }) => {
  await page.goto("/");

  const tabIds = ["import", "settings", "files", "lint", "contact"] as const;

  for (const activeId of tabIds) {
    await page.locator(`#tab-${activeId}`).click();

    for (const id of tabIds) {
      const panel = page.locator(`#panel-${id}`);
      if (id === activeId) {
        await expect(panel).toBeVisible();
        const box = await panel.boundingBox();
        expect(box?.width).toBeGreaterThan(0);
        expect(box?.height).toBeGreaterThan(0);
      } else {
        await expect(panel).toBeHidden();
      }
    }
  }
});

test("layout does not overflow horizontally at the app's default window size", async ({ page }) => {
  // 800x600 is the desktop app's configured default window size
  // (app/src-tauri/tauri.conf.json); it has no configured minimum, so a
  // user can resize below this, but this is the smallest size the app is
  // expected to look right at out of the box.
  await page.setViewportSize({ width: 800, height: 600 });
  await page.goto("/");

  // A layout that only works down to some minimum width tends to fail
  // silently by growing wider than the viewport instead of shrinking - this
  // is the cheapest possible check for that class of bug.
  const overflowX = await page.evaluate(() => document.documentElement.scrollWidth - document.documentElement.clientWidth);
  expect(overflowX).toBeLessThanOrEqual(1);

  await page.locator("#tab-settings").click();
  const settingsOverflowX = await page.evaluate(
    () => document.documentElement.scrollWidth - document.documentElement.clientWidth,
  );
  expect(settingsOverflowX).toBeLessThanOrEqual(1);
});

test("code viewer highlight overlay keeps blank lines the same height as their neighbours", async ({ page }) => {
  await page.goto("/");

  // Reproduces main.ts's updateCodeViewerEditHighlight() output directly
  // (one `.code-viewer__editor-line` span per source line, an empty span for
  // a blank line) against the real page's stylesheet, without needing the
  // Tauri backend that normally supplies the file contents. This is a
  // regression test for the exact bug styles.css documents fixing: a
  // `display: block` span with no content generates no line box at all
  // (zero height) unless min-height forces one, which would misalign this
  // overlay against the always-present blank lines in the textarea beneath
  // it.
  await page.evaluate(() => {
    const dialog = document.querySelector<HTMLDialogElement>("#code-viewer")!;
    dialog.showModal();
    document.querySelector("#code-viewer-view")!.setAttribute("hidden", "");
    document.querySelector("#code-viewer-editor")!.removeAttribute("hidden");
    const code = document.querySelector("#code-viewer-editor-highlight code")!;
    code.innerHTML = [
      "Scriptname Example extends Quest",
      "",
      "Function DoThing()",
      "EndFunction",
    ]
      .map((line) => `<span class="code-viewer__editor-line">${line}</span>`)
      .join("");
  });

  const lineHeights = await page.locator(".code-viewer__editor-line").evaluateAll((lines) =>
    lines.map((line) => line.getBoundingClientRect().height),
  );

  expect(lineHeights).toHaveLength(4);
  for (const height of lineHeights) {
    expect(height).toBeGreaterThan(5);
  }
  const [first, ...rest] = lineHeights;
  for (const height of rest) {
    expect(height).toBeCloseTo(first, 0);
  }
});
