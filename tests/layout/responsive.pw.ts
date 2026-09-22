import { test, expect, type Page } from "@playwright/test";
import { mockDesktop } from "./desktop-fixture";

async function fits(page: Page, selector: string) {
  const overflow = await page.locator(selector).evaluateAll((elements) =>
    elements
      .filter((el) => el.clientWidth > 0 && el.scrollWidth > el.clientWidth + 1)
      .map((el) => ({
        class: el.className,
        width: el.clientWidth,
        content: el.scrollWidth,
      })),
  );
  expect(overflow, `Horizontal overflow in ${selector}`).toEqual([]);
}

// CSS viewport dimensions, including constrained space at common display scales.
const sizes = [
  [900, 680],
  [1100, 720],
  [1101, 720],
  [1366, 768],
  [1440, 960],
  [1920, 1080],
  [2560, 1440],
  [720, 480],
];
for (const [width, height] of sizes) {
  test(`${width}x${height}: archive, detail, settings and search`, async ({
    page,
  }, testInfo) => {
    await page.setViewportSize({ width, height });
    await mockDesktop(
      page,
      width % 2 ? "zh-CN" : "en",
      width === 1440 ? "light" : "dark",
    );
    const errors: string[] = [];
    page.on("pageerror", (error) => errors.push(error.message));
    await page.goto("/");
    await expect(page.locator(".archive-match")).toHaveCount(1);
    await fits(
      page,
      "html, .workspace, .page-scroll, .archive-match, .page-toolbar, .play-filters",
    );
    const bounds = await page.locator(".archive-match").evaluate((el) => {
      const row = el.getBoundingClientRect();
      return [...el.children].every((child) => {
        const box = child.getBoundingClientRect();
        return box.left >= row.left && box.right <= row.right + 1;
      });
    });
    expect(bounds).toBe(true);
    await page.screenshot({ path: testInfo.outputPath("archive.png") });
    await page.locator(".archive-match-main").click();
    await expect(page.locator(".detail-page")).toBeVisible();
    await fits(
      page,
      "html, .workspace, .match-heading, .detail-tabs, .detail-tab-content",
    );
    // Dense data may scroll locally, but must not widen the application.
    await expect(page.locator(".scoreboard-scroll")).toBeVisible();
    await page.screenshot({ path: testInfo.outputPath("detail.png") });
    if (width < 1400) await page.locator(".notes-toggle").click();
    await expect(page.locator(".notes-panel")).toBeVisible();
    await page.screenshot({ path: testInfo.outputPath("notes.png") });
    await page.keyboard.press("Escape");
    // Reload to leave the review cleanly even when the drawer is inline.
    await page.reload();
    await page.locator(".sidebar-bottom .nav-button").click();
    await expect(page.locator(".settings-content")).toBeVisible();
    await fits(page, "html, .page-scroll, .setting-row");
    await page.screenshot({ path: testInfo.outputPath("settings.png") });
    await page.locator(".sidebar nav .nav-button").nth(2).click();
    await expect(page.locator(".query-form")).toBeVisible();
    await fits(page, "html, .page-scroll, .query-form");
    expect(errors).toEqual([]);
  });
}

test("match reflows inside a narrow container on a wide window", async ({
  page,
}) => {
  await page.setViewportSize({ width: 1920, height: 1080 });
  await mockDesktop(page, "ja", "light");
  await page.goto("/");
  const row = page.locator(".archive-match");
  await expect(row).toBeVisible();
  await row.evaluate((el) => ((el as HTMLElement).style.width = "560px"));
  await fits(page, ".archive-match");
  const main = await page.locator(".archive-match-main").boundingBox();
  const items = await page.locator(".archive-match-items").boundingBox();
  expect(items!.y).toBeGreaterThanOrEqual(main!.y + main!.height);
});

test("resizing switches sidebar and notes modes without losing a draft", async ({
  page,
}) => {
  await page.setViewportSize({ width: 1440, height: 960 });
  await mockDesktop(page, "ko", "dark");
  await page.goto("/");
  await page.locator(".archive-match-main").click();
  // Start from the actual visible state, including closed drawers.
  if (!(await page.locator(".notes-panel").isVisible()))
    await page.locator(".notes-toggle").click();
  await page
    .locator(".notes-panel textarea")
    .fill("Keep this draft across window sizes");
  await page.setViewportSize({ width: 1399, height: 768 });
  await expect(page.locator(".review-notes-drawer")).toHaveClass(
    /fui-OverlayDrawer/,
  );
  await expect(page.locator(".notes-panel textarea")).toHaveValue(
    "Keep this draft across window sizes",
  );
  await page.setViewportSize({ width: 1400, height: 900 });
  await expect(page.locator(".review-notes-drawer")).toHaveClass(
    /fui-InlineDrawer/,
  );
  await expect(page.locator(".notes-panel textarea")).toHaveValue(
    "Keep this draft across window sizes",
  );
  await page.setViewportSize({ width: 1100, height: 720 });
  await expect(page.locator(".app-shell")).toHaveAttribute(
    "data-compact-sidebar",
    "true",
  );
  await page.setViewportSize({ width: 1101, height: 720 });
  await expect(page.locator(".app-shell")).not.toHaveAttribute(
    "data-compact-sidebar",
  );
});

for (const scale of [1.25, 1.5]) {
  test(`1080p effective viewport at ${scale * 100}% display scale`, async ({
    browser,
  }, testInfo) => {
    const context = await browser.newContext({
      viewport: {
        width: Math.floor(1920 / scale),
        height: Math.floor(1080 / scale),
      },
      deviceScaleFactor: scale,
    });
    try {
      const page = await context.newPage();
      await mockDesktop(page, "zh-CN", "light");
      await page.goto("http://127.0.0.1:1420");
      await expect(page.locator(".archive-match")).toBeVisible();
      await fits(page, "html, .page-scroll, .archive-match, .page-toolbar");
      await page.screenshot({ path: testInfo.outputPath("scaled.png") });
    } finally {
      await context.close();
    }
  });
}
