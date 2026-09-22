import { test, expect } from "@playwright/test";
import { mockDesktop } from "./desktop-fixture";
import type { ReviewGame } from "../../src/generated/library/ReviewGame";
import type { LiveWorkspace } from "../../src/generated/live/LiveWorkspace";

test("timeline failures remain retryable and recover the item timeline", async ({ page }, testInfo) => {
  await page.setViewportSize({ width: 900, height: 900 });
  await mockDesktop(page, "zh-CN", "dark", "rich");
  await page.goto("/");
  await expect(page.locator(".archive-match")).toHaveCount(6);
  await page.evaluate(async () => {
    const api = (window as unknown as {
      __TAURI_INTERNALS__: { invoke: (command: string, args?: Record<string, unknown>) => Promise<unknown> };
    }).__TAURI_INTERNALS__;
    const invoke = api.invoke;
    const workspace = await invoke("live_workspace") as LiveWorkspace;
    workspace.status.state = "connected";
    let attempts = 0;
    api.invoke = async (command, args) => {
      if (command === "saved_review_game" || command === "open_review_game") {
        const game = await invoke(command, args) as ReviewGame;
        return { ...game, timeline: null, timelineError: null };
      }
      if (command === "ensure_review_timeline") {
        attempts++;
        const game = await invoke("saved_review_game", args) as ReviewGame;
        if (attempts === 1) return { ...game, timeline: null, timelineError: "game.invalidTimeline" };
        if (attempts === 2) throw { code: "client.NotFound" };
        return game;
      }
      return invoke(command, args);
    };
  });
  await page.locator(".archive-note-preview").first().click();
  const retry = page.locator(".review-feedback").getByRole("button", { name: "重试", exact: true });
  await expect(page.getByText("客户端返回的时间线格式不符合预期。", { exact: false })).toBeVisible({ timeout: 10_000 });
  await expect(retry).toBeEnabled();
  await page.screenshot({ path: testInfo.outputPath("timeline-retry.png"), animations: "disabled" });
  await retry.click();
  await expect(retry).toBeEnabled();
  await expect(page.locator(".review-feedback")).not.toContainText("客户端返回的时间线格式不符合预期。");
  await expect(page.locator(".review-feedback")).toContainText("客户端暂未提供这项数据，请稍后重试。");
  await retry.click();
  await expect(retry).toHaveCount(0);
  await page.getByRole("tab", { name: "出装时间线", exact: true }).click();
  await expect(page.locator(".timeline-canvas")).toBeVisible();
});

for (const code of ["library.storageError", "ui.conflict"]) {
  test(`pending changes recover from structured ${code}`, async ({ page }) => {
    await mockDesktop(page, "zh-CN", "light", "rich");
    await page.goto("/");
    await page.getByRole("button", { name: "关注动态", exact: true }).click();
    await page.getByRole("tab", { name: "玩家档案", exact: true }).click();
    await page.evaluate((code) => {
      const api = (
        window as unknown as {
          __TAURI_INTERNALS__: {
            invoke: (
              command: string,
              args?: Record<string, unknown>,
            ) => Promise<unknown>;
          };
        }
      ).__TAURI_INTERNALS__;
      const invoke = api.invoke;
      let fail = true;
      api.invoke = async (command, args) => {
        if (command === "save_live_ui" && fail) {
          fail = false;
          throw { code };
        }
        return invoke(command, args);
      };
    }, code);
    const profile = page
      .locator(".live-profile")
      .filter({ hasText: "一起排位的朋友" });
    await profile
      .getByRole("button", { name: "解除关联", exact: true })
      .click();
    const retry = page.getByRole("button", { name: "重试保存", exact: true });
    if (code === "library.storageError") {
      await expect(retry).toBeVisible();
      await retry.click();
    }
    await expect
      .poll(() =>
        page.evaluate(
          () =>
            (
              window as unknown as { testCalls: { command: string }[] }
            ).testCalls.filter((c) => c.command === "save_live_ui").length,
        ),
      )
      .toBe(1);
    if (code === "ui.conflict") {
      expect(
        await page.evaluate(() =>
          (
            window as unknown as { testCalls: { command: string }[] }
          ).testCalls.some((c) => c.command === "live_workspace"),
        ),
      ).toBe(true);
    }
    await expect(retry).toHaveCount(0);
    await expect(profile.locator(".live-account-binding")).toHaveCount(0);
  });
}
