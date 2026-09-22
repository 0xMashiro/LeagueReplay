import { test, expect } from "@playwright/test";
import { mockDesktop } from "./desktop-fixture";

test("status-only polling preserves archive records and current filters", async ({
  page,
}) => {
  await mockDesktop(page, "zh-CN", "light", "rich");
  await page.goto("/");
  await expect(page.locator(".archive-match")).toHaveCount(6);
  await page.locator(".play-filters input").fill("HN1_1");
  await expect(page.locator(".archive-match")).toHaveCount(1);
  await page.evaluate(() => {
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
    const original = api.invoke;
    api.invoke = async (command, args) => {
      const result = await original(command, args);
      if (command !== "sync_workspace") return result;
      const update = result as {
        cursor: number;
        status: Record<string, unknown>;
      };
      return {
        cursor: update.cursor,
        reset: false,
        removed: [],
        workspace: null,
        status: {
          ...update.status,
          message: "状态已刷新",
          lastCheckedAt: 4000,
        },
      };
    };
  });
  await expect(page.locator(".statusbar")).toContainText("状态已刷新", {
    timeout: 7000,
  });
  await expect(page.locator(".play-filters input")).toHaveValue("HN1_1");
  await expect(page.locator(".archive-match")).toHaveCount(1);
  await page.locator(".play-filters input").fill("");
  await expect(page.locator(".archive-match")).toHaveCount(6);
});
