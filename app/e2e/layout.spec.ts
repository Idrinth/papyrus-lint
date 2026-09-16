import { expect, test, type Page } from "@playwright/test";

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

test("lint progress bar is actually hidden when idle, not just marked hidden", async ({ page }) => {
  // Regression test for a bug jsdom can't see: `.lint-progress` used to set
  // `display: flex` unconditionally, which (being an author-stylesheet rule)
  // outranks the browser's default `[hidden] { display: none }` at equal
  // specificity. hideLintProgress()/scheduleHideLintProgress() were setting
  // the `hidden` attribute correctly the whole time - the element just never
  // actually disappeared, so the finished "N / N files" bar looked stuck
  // forever instead of hiding after its grace period.
  await page.goto("/");

  const progress = page.locator("#lint-progress");
  await expect(progress).toBeHidden();

  await page.evaluate(() => {
    document.querySelector<HTMLElement>("#lint-progress")!.hidden = false;
  });
  await expect(progress).toBeVisible();

  await page.evaluate(() => {
    document.querySelector<HTMLElement>("#lint-progress")!.hidden = true;
  });
  await expect(progress).toBeHidden();
});

test("config picker is actually hidden after a choice is made", async ({ page }) => {
  await page.goto("/");

  const picker = page.locator("#config-picker");
  await expect(picker).toBeHidden();

  await page.evaluate(() => {
    document.querySelector<HTMLDialogElement>("#config-picker")!.showModal();
  });
  await expect(picker).toBeVisible();

  await page.evaluate(() => {
    document.querySelector<HTMLDialogElement>("#config-picker")!.close();
  });
  await expect(picker).toBeHidden();
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

async function openTallCodeViewer(page: Page, mode: "view" | "edit") {
  await page.goto("/");
  await page.evaluate((nextMode) => {
    const lines = Array.from({ length: 200 }, (_, index) => `Scriptname Line${index} extends Quest`);
    const dialog = document.querySelector<HTMLDialogElement>("#code-viewer")!;
    const view = document.querySelector<HTMLElement>("#code-viewer-view")!;
    const editor = document.querySelector<HTMLElement>("#code-viewer-editor")!;
    const textarea = document.querySelector<HTMLTextAreaElement>("#code-viewer-editor-textarea")!;
    const highlight = document.querySelector("#code-viewer-editor-highlight code")!;
    const source = lines.join("\n");

    view.innerHTML =
      "<table class=\"code-viewer__table\"><tbody>" +
      lines
        .map(
          (line, index) =>
            `<tr><td class="code-viewer__line-number">${index + 1}</td>` +
            `<td class="code-viewer__line-code">${line}</td></tr>`,
        )
        .join("") +
      "</tbody></table>";
    textarea.value = source;
    highlight.innerHTML = lines.map((line) => `<span class="code-viewer__editor-line">${line}</span>`).join("");

    view.hidden = nextMode !== "view";
    editor.hidden = nextMode !== "edit";
    dialog.showModal();
  }, mode);
}

test("fullscreen code viewer covers the viewport so the page behind cannot show a scrollbar", async ({ page }) => {
  // Regression: `.code-viewer--fullscreen` used 100vw/100vh without resetting
  // the UA dialog's auto margins, so the box sat offset from the viewport
  // and the document's scrollbar (the element behind the dialog) was the
  // one on the right edge. Dragging it scrolled the page, not the viewer.
  await openTallCodeViewer(page, "view");

  await page.locator("#code-viewer-fullscreen").click();

  const geometry = await page.evaluate(() => {
    const dialog = document.querySelector<HTMLDialogElement>("#code-viewer")!;
    const box = dialog.getBoundingClientRect();
    return {
      x: box.x,
      y: box.y,
      width: box.width,
      height: box.height,
      viewportWidth: window.innerWidth,
      viewportHeight: window.innerHeight,
      htmlOverflow: getComputedStyle(document.documentElement).overflow,
    };
  });

  expect(geometry.x).toBeCloseTo(0, 0);
  expect(geometry.y).toBeCloseTo(0, 0);
  expect(geometry.width).toBeCloseTo(geometry.viewportWidth, 0);
  expect(geometry.height).toBeCloseTo(geometry.viewportHeight, 0);
  expect(geometry.htmlOverflow).toBe("hidden");

  const stage = page.locator(".code-viewer__stage");
  await expect(stage).toBeVisible();
  const scrolled = await stage.evaluate((el) => {
    el.scrollTop = 400;
    return el.scrollTop;
  });
  expect(scrolled).toBeGreaterThan(0);

  const pageScroll = await page.evaluate(() => document.documentElement.scrollTop);
  expect(pageScroll).toBe(0);
});

test("edit-mode scrollbar at the dialog edge belongs to the textarea, not the view behind it", async ({ page }) => {
  await openTallCodeViewer(page, "edit");
  await page.locator("#code-viewer-fullscreen").click();

  const hit = await page.evaluate(() => {
    const dialog = document.querySelector<HTMLDialogElement>("#code-viewer")!;
    const stage = document.querySelector<HTMLElement>(".code-viewer__stage")!;
    const highlight = document.querySelector<HTMLElement>("#code-viewer-editor-highlight")!;
    const box = dialog.getBoundingClientRect();
    const atRight = document.elementFromPoint(box.right - 8, box.top + box.height / 2);
    return {
      tag: atRight?.tagName ?? null,
      id: atRight instanceof HTMLElement ? atRight.id : null,
      stageOverflow: getComputedStyle(stage).overflow,
      highlightPointerEvents: getComputedStyle(highlight).pointerEvents,
    };
  });

  expect(hit.tag).toBe("TEXTAREA");
  expect(hit.id).toBe("code-viewer-editor-textarea");
  expect(hit.stageOverflow).toBe("hidden");
  expect(hit.highlightPointerEvents).toBe("none");

  const textarea = page.locator("#code-viewer-editor-textarea");
  const scrolled = await textarea.evaluate((el) => {
    el.scrollTop = 400;
    return el.scrollTop;
  });
  expect(scrolled).toBeGreaterThan(0);
});
