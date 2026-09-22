import { describe, expect, it } from "vitest";
import { indexPlay, filterPlay, initialPlayFilters } from "./play-query";
import { resourceData } from "../resources";
import type { LiveWorkspace } from "../generated/live/LiveWorkspace";

function fixture(count: number): LiveWorkspace {
  const start = new Date("2026-09-01T12:00:00").getTime();
  return {
    status: {
      state: "offline",
      message: "",
      phase: null,
      account: null,
      activeGameId: null,
      lastCheckedAt: 0,
    },
    accounts: [
      {
        id: "HN1:a",
        platform: "HN1",
        puuid: "a",
        summonerId: "1",
        riotId: "Owner#TEST",
      },
    ],
    matches: Array.from({ length: count }, (_, i) => ({
      id: `HN1_${i}`,
      observationId: `observation-${i}`,
      platform: "HN1",
      gameId: String(i),
      accountId: "HN1:a",
      segmentId: `segment-${Math.floor(i / 10)}`,
      firstSeen: start + i * 1000,
      lastSeen: start + i * 1000,
      championId: 103,
      queueName: "Ranked",
      state: "ready",
      automatic: i % 2 === 0,
      manual: i % 2 !== 0,
      dataError: null,
      game: {
        startedAt: start + i * 1000,
        duration: 1800,
        queueId: 420,
        version: "16.18",
        participants: ["a", "b", "c"].map((puuid, p) => ({
          id: p + 1,
          puuid,
          name: puuid + "#TEST",
          team: p === 2 ? 200 : 100,
          championId: 103,
          placement: null,
          role: "",
          win: i % 2 === 0,
          kills: 3,
          deaths: 2,
          assists: 4,
          cs: 0,
          gold: 0,
          damage: 0,
          items: [],
          doubleKills: 0,
          tripleKills: 0,
          quadraKills: 0,
          pentaKills: 0,
        })),
      },
    })),
    sessions: Array.from({ length: Math.ceil(count / 10) }, (_, i) => ({
      id: `session-${i}`,
      title: `Session ${i}`,
      startedAt: start,
      endedAt: start + 10000,
      endReason: "import",
      segments: [
        {
          id: `segment-${i}`,
          accountId: "HN1:a",
          matchIds: Array.from(
            { length: Math.min(10, count - i * 10) },
            (_, j) => `observation-${i * 10 + j}`,
          ),
        },
      ],
    })),
    ui: {
      myAccounts: [],
      notes: Array.from({ length: count }, (_, i) => ({
        id: `note-${i}`,
        matchId: `HN1_${i}`,
        body: `Lesson ${i}`,
        tags: ["review"],
        updatedAt: "2026-09-01",
      })),
      reviewed: [],
      theme: "light",
      players: [
        { id: "friend", name: "Friend", accountIds: ["HN1:b"] },
        { id: "opponent", name: "Opponent", accountIds: ["HN1:c"] },
      ],
      premades: [{ matchId: "HN1_1", accountId: "HN1:b" }],
      identityLinks: [],
    },
    uiRevision: 0,
    idleMinutes: 60,
  };
}

const ids = (result: ReturnType<typeof filterPlay>) =>
  [...result.recordsBySegment.values()].flat().map((r) => r.id);

describe("indexed play filters", () => {
  it("preserves text, date, result, source, and teammate/premade semantics", () => {
    const view = fixture(4);
    const index = indexPlay(view, "en", resourceData());
    const run = (changes = {}, player = "all", premade = false) =>
      ids(
        filterPlay(
          index,
          { ...initialPlayFilters, ...changes },
          player,
          premade,
          "all",
        ),
      );
    expect(run({ query: "  LESSON 1  " })).toEqual(["HN1_1"]);
    expect(
      run({
        query: "Owner#test",
        result: "win",
        source: "automatic",
        from: "2026-09-01",
        to: "2026-09-01",
      }),
    ).toEqual(["HN1_0", "HN1_2"]);
    expect(run({ from: "2026-09-02" })).toEqual([]);
    expect(run({}, "friend")).toHaveLength(4);
    expect(run({}, "friend", true)).toEqual(["HN1_1"]);
    expect(run({}, "opponent")).toEqual([]);
    expect(index.noteCount).toBe(4);
    expect(index.wins).toBe(2);
    expect(index.accountIds.size).toBe(1);
  });

  it("filters without reading match summary again and rebuilds after annotation changes", () => {
    const view = fixture(2);
    const index = indexPlay(view, "en", resourceData());
    for (const record of view.matches)
      Object.defineProperty(record, "game", {
        get: () => {
          throw Error("query reparsed summary");
        },
      });
    expect(
      ids(
        filterPlay(
          index,
          { ...initialPlayFilters, query: "lesson 0" },
          "friend",
          false,
          "all",
        ),
      ),
    ).toEqual(["HN1_0"]);
    const changed = fixture(2);
    changed.ui.notes[0].body = "Updated annotation";
    changed.ui.identityLinks = [
      {
        id: "override",
        playerId: "opponent",
        accountId: "HN1:b",
        matchId: "HN1_0",
        from: null,
        to: null,
      },
    ];
    const updated = indexPlay(changed, "en", resourceData());
    expect(
      ids(
        filterPlay(
          updated,
          { ...initialPlayFilters, query: "updated" },
          "opponent",
          false,
          "all",
        ),
      ),
    ).toEqual(["HN1_0"]);
  });

  it("retains open empty sessions and excludes ended sessions with no matching records", () => {
    const view = fixture(1);
    view.sessions.push({
      id: "open",
      title: "Open",
      startedAt: 0,
      endedAt: null,
      endReason: null,
      segments: [],
    });
    const index = indexPlay(view, "en", resourceData());
    expect(
      filterPlay(
        index,
        { ...initialPlayFilters, query: "missing" },
        "all",
        false,
        "all",
      ).sessions.map((s) => s.id),
    ).toEqual(["open"]);
    expect(
      filterPlay(
        index,
        initialPlayFilters,
        "all",
        false,
        "history",
      ).sessions.map((s) => s.id),
    ).toEqual(["session-0"]);
  });
});

it.skipIf(process.env.LEAGUEREPLAY_BENCH !== "1")(
  "benchmarks play indexing and repeated filtering",
  () => {
    for (const count of [1000, 10000]) {
      const view = fixture(count);
      const builds: number[] = [],
        queries: number[] = [];
      for (let sample = 0; sample < 5; sample++) {
        const start = performance.now();
        const index = indexPlay(view, "en", resourceData());
        builds.push(performance.now() - start);
        const queryStart = performance.now();
        for (const query of [
          "",
          "l",
          "le",
          "les",
          "lesson",
          "lesson 1",
          "lesson 10",
          "lesson 100",
          "review",
          "missing",
        ]) {
          filterPlay(
            index,
            { ...initialPlayFilters, query },
            "friend",
            false,
            "all",
          );
        }
        queries.push((performance.now() - queryStart) / 10);
        expect(index.rows).toHaveLength(count);
      }
      builds.sort((a, b) => a - b);
      queries.sort((a, b) => a - b);
      process.stdout.write(
        `matches=${count} index_median_ms=${builds[2].toFixed(2)} filter_mean_median_ms=${queries[2].toFixed(2)}\n`,
      );
    }
  },
  30000,
);
