import { test, expect } from "@playwright/test";
import { mockDesktop } from "./desktop-fixture";

test("backup uses file references, previews counts and handles a changed source", async ({
  page,
}) => {
  await mockDesktop(page, "zh-CN", "light", "rich");
  await page.goto("/");
  await page.evaluate(() => {
    const desktop = window as unknown as {
      finishRestore: () => void;
      testCalls: { command: string; args: Record<string, unknown> }[];
      __TAURI_INTERNALS__: {
        invoke: (
          command: string,
          args?: Record<string, unknown>,
        ) => Promise<unknown>;
      };
    };
    const original = desktop.__TAURI_INTERNALS__.invoke;
    let restores = 0;
    desktop.__TAURI_INTERNALS__.invoke = async (command, args = {}) => {
      if (
        !["choose_backup", "restore_backup", "export_backup"].includes(command)
      )
        return original(command, args);
      desktop.testCalls.push({ command, args });
      if (command === "export_backup") return "C:/Backups/current.jsonl";
      if (command === "choose_backup")
        return {
          path: "C:/Backups/large.jsonl",
          sourceFingerprint: "incoming",
          preview: {
            createdAt: 1789980000000,
            fingerprint: "current",
            counts: { games: 6500, notes: 2 },
          },
        };
      if (++restores === 1) {
        await new Promise<void>((resolve) => {
          desktop.finishRestore = resolve;
        });
        throw { code: "backup.changed" };
      }
      return "C:/Backups/before-restore.jsonl";
    };
  });
  await page.getByRole("button", { name: "设置", exact: true }).click();
  await page.getByRole("button", { name: "导出备份", exact: true }).click();
  await expect(
    page.getByText("C:/Backups/current.jsonl", { exact: false }),
  ).toBeVisible();
  await page
    .getByRole("button", { name: "打开备份文件…", exact: true })
    .click();
  const dialog = page.getByRole("dialog");
  await expect(dialog).toContainText("large.jsonl");
  await expect(dialog).toContainText("6500");
  const restore = dialog.getByRole("button", {
    name: "合并到当前归档",
    exact: true,
  });
  await restore.click();
  const pendingRestore = dialog.getByRole("button", {
    name: "正在保存…",
    exact: true,
  });
  await expect(pendingRestore).toBeDisabled();
  await expect(pendingRestore).toBeFocused();
  await page.evaluate(() =>
    (window as unknown as { finishRestore: () => void }).finishRestore(),
  );
  await expect(dialog).toContainText("预览后归档发生了变化");
  await restore.click();
  await expect(dialog).not.toBeVisible();
  const calls = await page.evaluate(() =>
    (
      window as unknown as {
        testCalls: { command: string; args: Record<string, unknown> }[];
      }
    ).testCalls.filter((c) => c.command === "restore_backup"),
  );
  expect(calls[1].args).toEqual({
    request: {
      path: "C:/Backups/large.jsonl",
      sourceFingerprint: "incoming",
      fingerprint: "current",
      merge: true,
    },
  });
});
