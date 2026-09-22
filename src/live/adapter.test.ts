import { describe, expect, it } from "vitest";
import type { ObservedMatch } from "../generated/live/ObservedMatch";
import type { ReviewGame } from "../generated/library/ReviewGame";
import { adaptMatch, adaptReview, withPlayer, championAlias } from "./adapter";
import { applyResources, resourceData } from "../resources";

const record: ObservedMatch = {
  id: "HN1_1",
  observationId: "observed-a",
  platform: "HN1",
  gameId: "1",
  accountId: "HN1:a",
  segmentId: "segment",
  firstSeen: 1000,
  lastSeen: 2000,
  championId: 103,
  queueName: "排位",
  state: "ready",
  automatic: true,
  manual: false,
  dataError: null,
  game: {
    startedAt: 1000,
    duration: 100,
    queueId: 0,
    version: "16.18",
    participants: ["a", "b", "c"].map((puuid, i) => ({
      id: i + 1,
      puuid,
      name: puuid.toUpperCase() + "#TEST",
      team: i === 2 ? 200 : 100,
      placement: null,
      championId: [103, 64, 254][i],
      role: "",
      win: i === 0 ? true : null,
      kills: i === 0 ? 3 : 0,
      deaths: 0,
      assists: i === 0 ? 1 : 0,
      cs: 0,
      gold: 0,
      damage: 0,
      items: [],
      doubleKills: 0,
      tripleKills: i === 0 ? 1 : 0,
      quadraKills: 0,
      pentaKills: 0,
    })),
  },
};
const review = (value: typeof record): ReviewGame => ({
  id: value.id,
  server: "TENCENT_HN1",
  account: {
    id: "HN1:a",
    platform: "HN1",
    puuid: "a",
    summonerId: "1",
    riotId: "A#TEST",
  },
  gameId: value.gameId,
  game: value.game!,
  queueName: "",
  timeline: [
    {
      id: "purchase",
      at: 42,
      participantId: 1,
      kind: "purchase",
      label: "purchase",
      itemId: 1001,
      restoredItemId: null,
      monster: "",
    },
    {
      id: "kill",
      at: 45,
      participantId: 1,
      kind: "kill",
      label: "kill",
      itemId: null,
      restoredItemId: null,
      monster: "",
    },
  ],
  source: "archive",
  timelineError: null,
  bookmarked: false,
});
describe("LCU projection", () => {
  it("refreshes numeric champion lookup when resources change", () => {
    const saved = resourceData();
    const previous = championAlias(103);
    try {
      applyResources({
        ...saved,
        catalog: {
          ...saved.catalog,
          champions: {
            Updated: {
              key: 103,
              name: "Updated",
              title: "Updated",
              bundled: false,
            },
          },
        },
      });
      expect(championAlias(103)).toBe("Updated");
      expect(championAlias(99999)).toBe("99999");
    } finally {
      applyResources(saved);
    }
    expect(championAlias(103)).toBe(previous);
  });
  it("list projection does not read timeline events", () => {
    const match = adaptMatch(record, "a")!;
    expect(match.events).toEqual([]);
    expect(match.timelineAvailable).toBe(false);
  });
  it("preserves normalized event times and multikill totals in the display model", () => {
    const match = adaptReview(review(record))!;
    expect(match.participantId).toBe(1);
    expect(match.multikills).toEqual({
      double: 0,
      triple: 1,
      quadra: 0,
      penta: 0,
    });
    expect(match.events.map((e) => [e.kind, e.at])).toEqual([
      ["purchase", 42],
      ["kill", 45],
    ]);
    expect(match.replay).toBe("unavailable");
    expect(adaptMatch({ ...record, state: "pending" }, "a")).toBeUndefined();
    expect(adaptMatch(record, "not-in-roster")).toBeUndefined();
  });
  it("retains item sales, undo direction and non-standard subteams", () => {
    const multi = review(structuredClone(record));
    multi.game.participants[0].team = 3;
    multi.game.participants[0].placement = 2;
    multi.timeline = [
      {
        id: "1",
        at: 1,
        participantId: 1,
        kind: "purchase",
        label: "purchase",
        itemId: 1001,
        restoredItemId: null,
        monster: "",
      },
      {
        id: "2",
        at: 2,
        participantId: 1,
        kind: "sale",
        label: "sale",
        itemId: 1001,
        restoredItemId: null,
        monster: "",
      },
      {
        id: "3",
        at: 3,
        participantId: 1,
        kind: "undo",
        label: "undoSale",
        itemId: 1001,
        restoredItemId: 1001,
        monster: "",
      },
      {
        id: "4",
        at: 4,
        participantId: 1,
        kind: "destroy",
        label: "destroy",
        itemId: 1001,
        restoredItemId: null,
        monster: "",
      },
    ];
    const match = adaptReview(multi)!;
    expect(match.participants.find((p) => p.id === 1)).toMatchObject({
      team: 3,
      placement: 2,
    });
    expect(match.events.map((e) => e.kind)).toEqual([
      "purchase",
      "sale",
      "undo",
      "destroy",
    ]);
    expect(match.events[2].restoredItemId).toBe(1001);
  });
  it("requires an explicit premade mark, and never treats opponents as teammates", () => {
    expect(withPlayer(record, "a", ["HN1:b"], [], false)).toBe(true);
    expect(withPlayer(record, "a", ["HN1:b"], [], true)).toBe(false);
    expect(
      withPlayer(
        record,
        "a",
        ["HN1:b"],
        [{ matchId: "HN1_1", accountId: "HN1:b" }],
        true,
      ),
    ).toBe(true);
    expect(
      withPlayer(
        record,
        "a",
        ["HN1:c"],
        [{ matchId: "HN1_1", accountId: "HN1:c" }],
        true,
      ),
    ).toBe(false);
    expect(withPlayer(record, "a", ["HN1:a"], [], false)).toBe(false);
  });
});
