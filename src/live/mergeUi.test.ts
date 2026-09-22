import { describe, expect, it } from "vitest";
import { mergeUi } from "./mergeUi";
import type { LiveUiState } from "../generated/live/LiveUiState";
const empty: LiveUiState = {
  myAccounts: [],
  notes: [],
  players: [],
  identityLinks: [],
  reviewed: [],
  premades: [],
  theme: "dark",
};
const note = (id: string, body: string) => ({
  id,
  matchId: "HN1_1",
  body,
  tags: [],
  updatedAt: "2026-09-21",
});
describe("annotation conflict recovery", () => {
  it("merges account shortcuts independently from notes and respects removal", () => {
    const saved = {
      server: "TENCENT_HN1",
      account: {
        id: "HN1:a",
        platform: "HN1",
        puuid: "a",
        summonerId: "1",
        riotId: "Player#TEST",
      },
    };
    const added = mergeUi(
      empty,
      { ...empty, myAccounts: [saved] },
      { ...empty, notes: [note("a", "review")] },
    );
    expect(added.conflicts).toEqual([]);
    expect(added.ui.myAccounts).toEqual([saved]);
    expect(added.ui.notes).toEqual([note("a", "review")]);
    const removed = mergeUi(
      added.ui,
      { ...added.ui, myAccounts: [] },
      { ...added.ui, notes: [note("a", "updated")] },
    );
    expect(removed.conflicts).toEqual([]);
    expect(removed.ui.myAccounts).toEqual([]);
    expect(removed.ui.notes[0].body).toBe("updated");
  });
  it("merges independent edits without discarding either writer", () => {
    const result = mergeUi(
      empty,
      { ...empty, notes: [note("a", "mine")] },
      { ...empty, notes: [note("b", "saved")], reviewed: ["HN1_2"] },
    );
    expect(result.conflicts).toEqual([]);
    expect(result.ui.notes.map((n) => n.id)).toEqual(["b", "a"]);
    expect(result.ui.reviewed).toEqual(["HN1_2"]);
  });
  it("requires a choice for edit versus delete and keeps unrelated changes", () => {
    const base = { ...empty, notes: [note("a", "original")] };
    const mine = { ...empty, notes: [note("a", "changed"), note("b", "new")] };
    const result = mergeUi(base, mine, empty, "saved");
    expect(result.conflicts.map((c) => c.field)).toEqual(["notes:a"]);
    expect(result.ui.notes).toEqual([note("b", "new")]);
  });
  it("does not silently link one account to two people", () => {
    const result = mergeUi(
      empty,
      { ...empty, players: [{ id: "p1", name: "One", accountIds: ["HN1:a"] }] },
      { ...empty, players: [{ id: "p2", name: "Two", accountIds: ["HN1:a"] }] },
    );
    expect(result.conflicts.some((c) => c.field === "players")).toBe(true);
    expect(result.ui.players).toHaveLength(1);
  });
});
