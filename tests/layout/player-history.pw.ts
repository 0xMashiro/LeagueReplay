import { test, expect, type Page } from "@playwright/test";
import { mockDesktop } from "./desktop-fixture";

async function queries(page: Page) {
  return page.evaluate(() =>
    (
      window as unknown as {
        testCalls: { command: string; args: Record<string, unknown> }[];
      }
    ).testCalls.filter((c) =>
      ["player_history_page", "search_player"].includes(c.command),
    ),
  );
}

test("history snapshot survives page changes and ignores a late response from an unmounted page", async ({
  page,
}) => {
  await mockDesktop(page, "zh-CN", "light", "rich");
  await page.goto("/");
  await page.locator(".segment-header .player-history-link").first().click();
  await expect(page.locator(".search-page .archive-match")).toHaveCount(6);
  await page.getByRole("button", { name: "设置", exact: true }).click();
  await expect(page.locator(".settings-page")).toBeVisible();
  await page.getByRole("button", { name: "战绩查询", exact: true }).click();
  await expect(page.locator(".search-page .archive-match")).toHaveCount(6);
  expect(await queries(page)).toHaveLength(1);

  await page.evaluate(() => {
    const host = window as unknown as {
      releaseHistory?: () => void;
      __TAURI_INTERNALS__: {
        invoke: (
          command: string,
          args?: Record<string, unknown>,
        ) => Promise<unknown>;
      };
    };
    const original = host.__TAURI_INTERNALS__.invoke;
    host.__TAURI_INTERNALS__.invoke = async (command, args) => {
      const result = await original(command, args);
      if (command !== "search_current_player") return result;
      host.__TAURI_INTERNALS__.invoke = original;
      return new Promise((resolve) => {
        host.releaseHistory = () =>
          resolve({ ...(result as object), games: [] });
      });
    };
  });
  await page.locator(".search-page .live-heading button").click();
  await expect
    .poll(() =>
      page.evaluate(
        () =>
          typeof (window as unknown as { releaseHistory?: unknown })
            .releaseHistory,
      ),
    )
    .toBe("function");
  await page.getByRole("button", { name: "设置", exact: true }).click();
  await expect(page.locator(".settings-page")).toBeVisible();
  await page.getByRole("button", { name: "战绩查询", exact: true }).click();
  await page.locator(".search-page .live-heading button").click();
  await expect(page.locator(".search-page .archive-match")).toHaveCount(6);
  await page.evaluate(async () => {
    (window as unknown as { releaseHistory: () => void }).releaseHistory();
    await new Promise(requestAnimationFrame);
  });
  await expect(page.locator(".search-page .archive-match")).toHaveCount(6);
});

test("account and participant links query the correct region and identity", async ({
  page,
}) => {
  await mockDesktop(page, "zh-CN", "light", "rich");
  await page.goto("/");
  const account = page.locator(".segment-header .player-history-link").nth(1);
  await expect(account).toHaveAttribute("aria-disabled", "false");
  await account.focus();
  await page.keyboard.press("Enter");
  await expect(page.locator(".search-page .archive-match")).toHaveCount(6);
  expect(await queries(page)).toEqual([
    {
      command: "player_history_page",
      args: { server: "TENCENT_HN1", puuid: "b", start: 0 },
    },
  ]);
  await page.locator(".archive-match-main").first().click();
  await page.locator(".scoreboard .player-history-link").nth(1).click();
  await expect(page.locator(".search-page .archive-match")).toHaveCount(6);
  expect((await queries(page)).at(-1)?.args).toEqual({
    server: "TENCENT_HN1",
    puuid: "player-1",
    start: 0,
  });
  await page.locator(".sidebar nav .nav-button").nth(1).click();
  await page.locator(".following-tabs [role=tab]").nth(1).click();
  await page
    .locator(".live-account-binding .player-history-link")
    .first()
    .click();
  await expect(page.locator(".search-page .archive-match")).toHaveCount(6);
  expect((await queries(page)).at(-1)?.args.puuid).toBe("a");
  await page.locator(".sidebar nav .nav-button").nth(1).click();
  await page.locator(".follow-management summary").click();
  await page.locator(".follow-status .player-history-link").click();
  await expect(page.locator(".search-page .archive-match")).toHaveCount(6);
});

test("unavailable region does not issue a history query", async ({ page }) => {
  await mockDesktop(page, "zh-CN", "light", "rich", false);
  await page.goto("/");
  const link = page.locator(".segment-header .player-history-link").first();
  await expect(link).toHaveAttribute("aria-disabled", "true");
  await link.focus();
  await page.keyboard.press("Enter");
  expect(await queries(page)).toEqual([]);
  await expect(page.locator(".play-page")).toBeVisible();
});

test("player navigation respects unsaved review notes", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await mockDesktop(page, "zh-CN", "light", "rich");
  await page.goto("/");
  await page.locator(".archive-match-main").first().click();
  await page.locator(".notes-panel textarea").fill("尚未保存的复盘");
  await page.locator(".scoreboard .player-history-link").nth(1).click();
  await expect(page.getByRole("dialog")).toBeVisible();
  expect(await queries(page)).toEqual([]);
  await page.getByRole("button", { name: "继续编辑", exact: true }).click();
  await expect(page.locator(".notes-panel textarea")).toHaveValue(
    "尚未保存的复盘",
  );
  await page.locator(".scoreboard .player-history-link").nth(1).click();
  await page
    .getByRole("button", { name: "放弃草稿并离开", exact: true })
    .click();
  await expect(page.locator(".search-page .archive-match")).toHaveCount(6);
  expect((await queries(page)).at(-1)?.args.puuid).toBe("player-1");
});

test("missing identity is blocked and a full Riot ID is a query fallback", async ({
  page,
}) => {
  await mockDesktop(page, "zh-CN", "light", "rich");
  await page.goto("/");
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
      if (command === "saved_review_game") {
        const game = structuredClone(result) as {
          game: {
            participants: { puuid: string | null; name: string | null }[];
          };
        };
        game.game.participants[1].puuid = null;
        game.game.participants[1].name = "旧昵称";
        game.game.participants[2].puuid = null;
        game.game.participants[2].name = "可查询昵称#TEST";
        return game;
      }
      return result;
    };
  });
  await page.locator(".archive-match-main").first().click();
  const missing = page.locator(".scoreboard .player-history-link").nth(1);
  await expect(missing).toHaveAttribute("aria-disabled", "true");
  await missing.focus();
  await page.keyboard.press("Enter");
  expect(await queries(page)).toEqual([]);
  await page.locator(".scoreboard .player-history-link").nth(2).click();
  await expect(page.locator(".search-page .archive-match")).toHaveCount(6);
  expect(await queries(page)).toEqual([
    {
      command: "search_player",
      args: { server: "TENCENT_HN1", riotId: "可查询昵称#TEST" },
    },
  ]);
});

test("refresh retains only selected IDs still present in typed history results", async ({
  page,
}) => {
  await mockDesktop(page, "zh-CN", "light", "rich");
  await page.goto("/");
  await page.locator(".segment-header .player-history-link").first().click();
  const rows = page.locator(".search-page .archive-match");
  await expect(rows).toHaveCount(6);
  await rows.first().getByRole("checkbox").check();
  await rows.last().getByRole("checkbox").check();
  await page.evaluate(() => {
    const host = window as unknown as {
      __TAURI_INTERNALS__: {
        invoke: (
          command: string,
          args?: Record<string, unknown>,
        ) => Promise<unknown>;
      };
    };
    const original = host.__TAURI_INTERNALS__.invoke;
    host.__TAURI_INTERNALS__.invoke = async (command, args) => {
      const result = await original(command, args);
      if (command !== "player_history_page") return result;
      const page = result as { games: { gameId: string }[] };
      return {
        ...page,
        games: page.games.filter((game) => game.gameId !== "1"),
      };
    };
  });
  await page.getByRole("button", { name: "刷新当前战绩", exact: true }).click();
  await expect(rows).toHaveCount(5);
  await expect(
    page.locator(".search-page .archive-match.is-selected"),
  ).toHaveCount(1);
  await expect(rows.last().getByRole("checkbox")).toBeChecked();
  await expect(page.locator(".selection-toolbar")).toContainText("1");
});
