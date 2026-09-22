import type { SearchResult } from "../../src/generated/library/SearchResult";
import type { Page } from "@playwright/test";
import type { LiveWorkspace } from "../../src/generated/live/LiveWorkspace";

// Deterministic IPC responses keep layout tests independent of the local
// archive and game client. No desktop commands leave the browser context.
export async function mockDesktop(
  page: Page,
  language: string,
  theme: string,
  mode: "basic" | "rich" | "empty" = "basic",
  queryAvailable = true,
) {
  await page.addInitScript(
    ({ language, theme, mode, queryAvailable }) => {
      localStorage.setItem("league-replay:language", language);
      localStorage.setItem("league-replay:theme", theme);
      const account = {
        id: "HN1:a",
        platform: "HN1",
        puuid: "a",
        summonerId: "1",
        riotId:
          mode === "rich" ? "晚风拾光#0921" : "LongPlayerNameForLayout#TEST",
      };
      const detail = {
        gameId: 1,
        queueId: 420,
        gameCreation: 1789980000000,
        gameDuration: 1824,
        gameVersion: "16.18",
        gameMode: "CLASSIC",
        participantIdentities: Array.from({ length: 10 }, (_, i) => ({
          participantId: i + 1,
          player: {
            puuid: i === 0 ? "a" : `player-${i}`,
            gameName:
              mode === "rich"
                ? [
                    "晚风拾光",
                    "暮色回廊",
                    "Riverpath",
                    "风止于此",
                    "青苔",
                    "Northbound",
                    "雨后长街",
                    "云间客",
                    "碎星",
                    "Juniper",
                  ][i]
                : `LongPlayerName${i}`,
            tagLine: "TEST",
          },
        })),
        participants: Array.from({ length: 10 }, (_, i) => ({
          participantId: i + 1,
          teamId: i < 5 ? 100 : 200,
          championId:
            mode === "rich"
              ? [103, 64, 254, 222, 99, 157, 103, 64, 222, 99][i]
              : 103,
          stats: {
            win: i < 5,
            kills: 12,
            deaths: 3,
            assists: 18,
            totalMinionsKilled: 210,
            goldEarned: 16000,
            totalDamageDealtToChampions: 30000,
            item0: 1001,
            item1: 3089,
            item2: 3157,
            item3: 3135,
            item4: 3118,
            item5: 4645,
            item6: 3363,
          },
        })),
      };
      const record = {
        id: "HN1_1",
        observationId: "observed-1",
        platform: "HN1",
        gameId: "1",
        accountId: account.id,
        segmentId: "segment",
        firstSeen: detail.gameCreation,
        lastSeen: detail.gameCreation,
        championId: 103,
        queueName: "排位",
        state: "ready",
        automatic: true,
        manual: false,
        dataError: null,
        detail,
      };
      const records =
        mode === "empty"
          ? []
          : mode === "rich"
            ? Array.from({ length: 6 }, (_, i) => ({
                ...record,
                id: `HN1_${i + 1}`,
                observationId: `observed-${i + 1}`,
                gameId: String(i + 1),
                accountId: ["HN1:a", "HN1:b", "HN1:c"][Math.floor(i / 2)],
                segmentId: `segment-${Math.floor(i / 2)}`,
                championId: [103, 157, 61, 103, 98, 222][i],
                detail: {
                  ...detail,
                  gameId: i + 1,
                  gameCreation: detail.gameCreation - i * 2400000,
                  participantIdentities: detail.participantIdentities.map(
                    (p, j) =>
                      j === 0
                        ? {
                            ...p,
                            player: {
                              ...p.player,
                              puuid: ["a", "b", "c"][Math.floor(i / 2)],
                            },
                          }
                        : p,
                  ),
                  participants: detail.participants.map((p, j) => ({
                    ...p,
                    championId:
                      j === 0 ? [103, 157, 61, 103, 98, 222][i] : p.championId,
                    stats: { ...p.stats, win: i % 3 !== 1 },
                  })),
                },
              }))
            : [record];
      const workspace: LiveWorkspace = {
        status: {
          state: "disconnected",
          message: "打开并登录游戏客户端后，会自动开始检测。",
          phase: null,
          account: null,
          activeGameId: null,
          lastCheckedAt: detail.gameCreation,
        },
        accounts:
          mode === "rich"
            ? [
                account,
                {
                  ...account,
                  id: "HN1:b",
                  puuid: "b",
                  riotId: "今天早点睡#0717",
                },
                {
                  ...account,
                  id: "HN1:c",
                  puuid: "c",
                  riotId: "沿途有光#0206",
                },
              ]
            : [account],
        matches: records.map(({ detail: game, ...record }) => ({
          ...record,
          game: {
            startedAt: game.gameCreation,
            duration: game.gameDuration,
            queueId: game.queueId,
            version: game.gameVersion,
            participants: game.participants.map((person) => {
              const player = game.participantIdentities.find(
                (p) => p.participantId === person.participantId,
              )!.player;
              const stats = person.stats;
              return {
                id: person.participantId,
                puuid: player.puuid,
                name: player.gameName + "#" + player.tagLine,
                team: person.teamId,
                championId: person.championId,
                placement: null,
                role: "",
                win: stats.win,
                kills: stats.kills,
                deaths: stats.deaths,
                assists: stats.assists,
                cs: stats.totalMinionsKilled,
                gold: stats.goldEarned,
                damage: stats.totalDamageDealtToChampions,
                items: [
                  stats.item0,
                  stats.item1,
                  stats.item2,
                  stats.item3,
                  stats.item4,
                  stats.item5,
                  stats.item6,
                ],
                doubleKills: 0,
                tripleKills: 0,
                quadraKills: 0,
                pentaKills: 0,
              };
            }),
          },
        })),
        sessions:
          mode === "empty"
            ? []
            : [
                {
                  id: "session",
                  title:
                    mode === "rich"
                      ? "晚间排位 · 中路练习"
                      : "Layout session with a long descriptive title",
                  startedAt:
                    detail.gameCreation - (mode === "rich" ? 5 * 2400000 : 0),
                  endedAt: detail.gameCreation + 1824000,
                  endReason: "manual",
                  segments:
                    mode === "rich"
                      ? [0, 1, 2].map((i) => ({
                          id: `segment-${i}`,
                          accountId: ["HN1:a", "HN1:b", "HN1:c"][i],
                          matchIds: records
                            .filter((r) => r.segmentId === `segment-${i}`)
                            .map((r) => r.observationId),
                        }))
                      : [
                          {
                            id: "segment",
                            accountId: account.id,
                            matchIds: records.map((r) => r.observationId),
                          },
                        ],
                },
              ],
        ui: {
          myAccounts:
            mode === "rich" ? [{ server: "TENCENT_HN1", account }] : [],
          notes:
            mode === "rich"
              ? [
                  {
                    id: "note-1",
                    matchId: "HN1_1",
                    body: "第三条小龙前先推中线，提前和打野一起占住河道。下次留意边线兵线的位置。",
                    tags: ["团战", "视野"],
                    at: 900,
                    updatedAt: "2026-09-21T16:45:00Z",
                  },
                ]
              : [],
          reviewed: mode === "rich" ? ["HN1_3"] : [],
          theme,
          players:
            mode === "rich"
              ? [
                  {
                    id: "player-1",
                    name: "一起排位的朋友",
                    accountIds: [account.id],
                  },
                  { id: "player-2", name: "中路练习搭档", accountIds: [] },
                ]
              : [],
          premades: [],
          identityLinks: [],
        },
        uiRevision: 0,
        idleMinutes: 30,
      };
      const entries = records.map((r, i) => ({
        id: r.id,
        server: "TENCENT_HN1",
        account,
        championId: [103, 64, 254, 222, 99, 157][i % 6],
        startedAt: detail.gameCreation - i * 2400000,
        duration: 1824 + i * 47,
        win: i % 3 !== 1,
        kills: 12 - i,
        deaths: 3 + i,
        assists: 18 - i,
        version: "16.18",
        bookmarked: i === 0,
        items: [1001, 3089, 3157, 3135, 3118, 4645, 3363],
        queueId: 420,
      }));
      const jobs =
        mode === "rich"
          ? [
              {
                id: "HN1_1",
                state: "ready",
                received: 12582912,
                total: 12582912,
                version: "16.18",
                error: null,
              },
              {
                id: "HN1_2",
                state: "downloading",
                received: 4718592,
                total: 13631488,
                version: "16.18",
                error: null,
              },
              {
                id: "HN1_3",
                state: "waiting",
                received: 0,
                total: null,
                version: "16.18",
                error: null,
              },
            ]
          : [];
      const calls: { command: string; args: Record<string, unknown> }[] = [];
      Object.assign(window, {
        testCalls: calls,
        isTauri: true,
        __TAURI_INTERNALS__: {
          transformCallback: () => 1,
          invoke: async (
            command: string,
            args: Record<string, unknown> = {},
          ) => {
            calls.push({ command, args });
            switch (command) {
              case "save_live_ui":
                workspace.ui = args.ui as LiveWorkspace["ui"];
                return ++workspace.uiRevision;
              case "live_workspace":
                return workspace;
              case "sync_workspace":
                return {
                  cursor: 1,
                  reset: true,
                  removed: [],
                  status: workspace.status,
                  workspace,
                };
              case "replay_jobs":
                return jobs;
              case "list_library":
                return mode !== "rich"
                  ? []
                  : entries.filter(
                      (e) =>
                        (!args.query ||
                          e.account.riotId.includes(String(args.query))) &&
                        (args.filter !== "saved" || e.bookmarked) &&
                        (args.filter !== "downloaded" || e.id === "HN1_1"),
                    );
              case "following_feed":
                return mode !== "rich"
                  ? []
                  : entries
                      .filter(
                        (e) =>
                          !args.unread || !workspace.ui.reviewed.includes(e.id),
                      )
                      .map((game) => ({ subscriptionId: "follow-1", game }));
              case "search_current_player":
              case "search_player":
              case "player_history_page":
                return {
                  account,
                  server: "TENCENT_HN1",
                  games: workspace.matches.map((r) => ({
                    gameId: r.gameId,
                    game: {
                      ...r.game!,
                      participants: r.game!.participants.map((p, i) =>
                        i === 0 ? { ...p, puuid: account.puuid } : p,
                      ),
                    },
                  })),
                  start: 0,
                  hasMore: false,
                  skippedGames: 0,
                  source: "lcu",
                } satisfies SearchResult;
              case "cached_resources":
                return null;
              case "desktop_ready":
              case "plugin:event|listen":
              case "plugin:event|unlisten":
                return 1;
              case "desktop_settings":
                return {
                  preferences: { closeToTray: false, language },
                  autostart: false,
                };
              case "replay_directory":
                return (
                  "C:/Users/Layout/Documents/" + "LongDirectoryName/".repeat(10)
                );
              case "following_state":
                return {
                  subscriptions:
                    mode !== "rich"
                      ? []
                      : [
                          {
                            id: "follow-1",
                            account,
                            server: "TENCENT_HN1",
                            label: "中路观察",
                            paused: false,
                            lastSynced: detail.gameCreation,
                            lastError: null,
                            limited: false,
                          },
                        ],
                  syncing: false,
                };
              case "query_regions":
                return [
                  {
                    id: "TENCENT_HN1",
                    platform: "HN1",
                    name: "艾欧尼亚",
                    englishName: "Ionia",
                    available: queryAvailable,
                    current: true,
                  },
                ];
              case "saved_review_game":
              case "open_review_game":
                return {
                  id: record.id,
                  server: "HN1",
                  account,
                  gameId: "1",
                  game: workspace.matches[0]!.game!,
                  queueName: "CLASSIC",
                  timeline: [
                    {
                      id: "purchase-1",
                      at: 65,
                      participantId: 1,
                      kind: "purchase",
                      label: "purchase",
                      itemId: 1001,
                      restoredItemId: null,
                      monster: "",
                    },
                    {
                      id: "purchase-2",
                      at: 360,
                      participantId: 1,
                      kind: "purchase",
                      label: "purchase",
                      itemId: 3089,
                      restoredItemId: null,
                      monster: "",
                    },
                    {
                      id: "kill-1",
                      at: 900,
                      participantId: 1,
                      kind: "kill",
                      label: "kill",
                      itemId: null,
                      restoredItemId: null,
                      monster: "",
                    },
                  ],
                  source: "observed",
                  timelineError: null,
                  bookmarked: false,
                };
              default:
                throw new Error(`Unmocked desktop command: ${command}`);
            }
          },
        },
      });
    },
    { language, theme, mode, queryAvailable },
  );
}
