import { test, expect } from "@playwright/test";
import { mockDesktop } from "./desktop-fixture";

test("following owns profiles and can reassign, create and unlink a player", async ({
  page,
}) => {
  await mockDesktop(page, "zh-CN", "light", "rich");
  await page.goto("/");
  await expect(page.locator(".sidebar nav .nav-button")).toHaveCount(4);
  await page.getByRole("button", { name: "关注动态", exact: true }).click();
  await expect(
    page.getByRole("tab", { name: "动态", exact: true }),
  ).toHaveAttribute("aria-selected", "true");
  await page.locator(".follow-management summary").click();
  await page.locator(".follow-status .associate-player").click();
  await page.getByRole("dialog").getByRole("combobox").click();
  await page.getByRole("option", { name: "中路练习搭档", exact: true }).click();
  await page.getByRole("button", { name: "保存", exact: true }).click();
  await expect(page.locator(".follow-status .associate-player")).toContainText(
    "中路练习搭档",
  );
  await page.getByRole("tab", { name: "玩家档案", exact: true }).click();
  const old = page
    .locator(".live-profile")
    .filter({ hasText: "一起排位的朋友" });
  const chosen = page
    .locator(".live-profile")
    .filter({ hasText: "中路练习搭档" });
  await expect(old.locator(".live-account-binding")).toHaveCount(0);
  await expect(chosen.locator(".live-account-binding")).toHaveCount(1);
  await page.getByRole("tab", { name: "动态", exact: true }).click();
  await page.locator(".follow-management summary").click();
  await page.locator(".follow-status .associate-player").click();
  await page.getByRole("dialog").getByRole("combobox").click();
  await page.getByRole("option", { name: "新建玩家", exact: true }).click();
  await expect(
    page.getByRole("button", { name: "保存", exact: true }),
  ).toBeDisabled();
  await page.getByRole("dialog").locator("input").fill("固定搭档");
  await page.getByRole("button", { name: "保存", exact: true }).click();
  await expect(page.locator(".follow-status .associate-player")).toContainText(
    "固定搭档",
  );
  await page.getByRole("tab", { name: "玩家档案", exact: true }).click();
  await expect(page.locator(".live-profile")).toHaveCount(3);
  const created = page.locator(".live-profile").filter({ hasText: "固定搭档" });
  await expect(created.locator(".live-account-binding")).toHaveCount(1);
  await created.getByRole("button", { name: "解除关联", exact: true }).click();
  await expect(created.locator(".live-account-binding")).toHaveCount(0);
  const saved = await page.evaluate(
    () =>
      (
        window as unknown as {
          testCalls: {
            command: string;
            args: {
              ui?: { players: { name: string; accountIds: string[] }[] };
            };
          }[];
        }
      ).testCalls
        .filter((c) => c.command === "save_live_ui")
        .at(-1)?.args.ui,
  );
  expect(saved?.players.every((p) => !p.accountIds.includes("HN1:a"))).toBe(
    true,
  );
});
