import { test, expect, type Page } from "@playwright/test";
import { mockDesktop } from "./desktop-fixture";

async function noPageOverflow(page: Page) {
  const overflow = await page
    .locator(
      "html, .page-scroll, .view-controls, .query-panel, .setting-row, .live-profile, .archive-match",
    )
    .evaluateAll((elements) =>
      elements
        .filter(
          (el) => el.clientWidth > 0 && el.scrollWidth > el.clientWidth + 1,
        )
        .map((el) => el.className || el.tagName),
    );
  expect(overflow).toEqual([]);
}

for (const [width, theme, language] of [
  [1440, "light", "zh-CN"],
  [900, "dark", "en"],
] as const) {
  test(`populated workspace ${width} ${theme}`, async ({ page }, testInfo) => {
    await page.setViewportSize({ width, height: 900 });
    await mockDesktop(page, language, theme, "rich");
    const errors: string[] = [];
    page.on("pageerror", (error) => errors.push(error.message));
    const capture = async (name: string) => {
      await noPageOverflow(page);
      await page.mouse.move(0, 0);
      await page.screenshot({
        path: testInfo.outputPath(`${name}.png`),
        animations: "disabled",
      });
    };
    await page.goto("/");
    await expect(page.locator(".archive-match")).toHaveCount(6);
    await expect(page.locator(".archive-overview dd")).toHaveText([
      "1",
      "6",
      "67%",
      "1",
    ]);
    await capture("play");
    const noted = page
      .locator(".archive-match")
      .filter({ has: page.locator(".archive-note-preview") });
    await expect(noted).toHaveCount(1);
    expect(
      await noted.evaluate((row) => {
        const preview = row
          .querySelector(".archive-note-preview")!
          .getBoundingClientRect();
        return [
          ".archive-match-main",
          ".archive-match-items",
          ".archive-match-actions",
        ].every(
          (selector) =>
            row.querySelector(selector)!.getBoundingClientRect().bottom <=
            preview.top,
        );
      }),
    ).toBe(true);
    await noted.locator(".archive-note-preview").focus();
    await page.keyboard.press("Enter");
    await expect(page.locator(".scoreboard")).toBeVisible();
    await page.locator(".detail-breadcrumb button").first().click();
    await expect(page.locator(".archive-match")).toHaveCount(6);
    await page.locator(".sidebar nav .nav-button").nth(1).click();
    await expect(page.locator(".archive-match")).toHaveCount(6);
    await page.locator(".follow-management summary").click();
    await expect(page.locator(".follow-status")).toBeVisible();
    await capture("following");
    await page.locator(".following-tabs [role=tab]").nth(1).click();
    await expect(page.locator(".live-profile")).toHaveCount(2);
    await capture("players");
    await page.locator(".sidebar nav .nav-button").nth(2).click();
    await page.locator(".query-form input").fill("晚风拾光#0921");
    await page.locator(".query-form button[type=submit]").click();
    await expect(page.locator(".archive-match")).toHaveCount(6);
    await expect(page.locator(".selection-toolbar button")).toHaveCount(0);
    await page.locator(".selection-toolbar input[type=checkbox]").check();
    await expect(page.locator(".selection-toolbar button")).toHaveCount(2);
    await page.locator(".selection-toolbar input[type=checkbox]").uncheck();
    await capture("search");
    await page.locator(".sidebar nav .nav-button").nth(3).click();
    await expect(page.locator(".archive-match")).toHaveCount(6);
    await capture("library");
    await page.locator(".library-page [role=tab]").nth(3).click();
    await expect(page.locator(".replay-task")).toHaveCount(3);
    await capture("downloads");
    await page.locator(".library-page [role=tab]").nth(4).click();
    await expect(page.locator(".review-note")).toHaveCount(1);
    await capture("notes");
    await page.locator(".review-note button").click();
    await expect(page.locator(".scoreboard")).toBeVisible();
    await capture("detail");
    await page.locator(".detail-tabs [role=tab]").nth(1).click();
    await expect(page.locator(".timeline-scroll")).toBeVisible();
    await capture("timeline");
    await page.locator(".sidebar-bottom .nav-button").click();
    await expect(page.locator(".settings-content")).toBeVisible();
    await capture("settings");
    expect(errors).toEqual([]);
  });
}

test("empty pages offer a path forward", async ({ page }, testInfo) => {
  await page.setViewportSize({ width: 900, height: 680 });
  await mockDesktop(page, "zh-CN", "dark", "empty");
  await page.goto("/");
  await expect(page.locator(".empty-state button")).toBeVisible();
  await expect(page.locator(".archive-overview dd")).toHaveText([
    "0",
    "0",
    "—",
    "0",
  ]);
  await page.screenshot({ path: testInfo.outputPath("empty-play.png") });
  for (const index of [1, 3]) {
    await page.locator(".sidebar nav .nav-button").nth(index).click();
    await expect(page.locator(".empty-state")).toBeVisible();
    await noPageOverflow(page);
  }
});

test("advanced player filters reset with match filters", async ({ page }) => {
  await mockDesktop(page, "zh-CN", "light", "rich");
  await page.goto("/");
  await page.getByRole("button", { name: "筛选", exact: true }).click();
  await page.getByRole("combobox", { name: "筛选同队玩家" }).click();
  await page.getByRole("option", { name: "中路练习搭档", exact: true }).click();
  await expect(page.locator(".archive-match")).toHaveCount(0);
  await page
    .locator(".play-filter-panel")
    .getByRole("button", { name: "清除筛选", exact: true })
    .click();
  await expect(page.locator(".archive-match")).toHaveCount(6);
  await expect(
    page.getByRole("combobox", { name: "筛选同队玩家" }),
  ).toContainText("全部玩家");
});
