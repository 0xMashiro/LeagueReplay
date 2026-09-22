import { describe, expect, it } from "vitest";
import { groupPurchases, parseTime } from "./archive";
import type { MatchEvent } from "./types";

describe("timeline and annotations", () => {
  it("keeps interleaved players separate and prevents sliding purchase groups", () => {
    const purchase = (
      id: string,
      participantId: number,
      at: number,
    ): MatchEvent => ({
      id,
      participantId,
      at,
      kind: "purchase",
      itemId: 1056,
      label: "购买装备",
    });
    const groups = groupPurchases([
      purchase("a", 1, 100),
      purchase("b", 2, 105),
      purchase("c", 1, 117),
      purchase("d", 1, 135),
      { id: "kill", participantId: 1, at: 110, kind: "kill", label: "击杀" },
    ]);
    expect(groups.map((group) => group.map((event) => event.id))).toEqual([
      ["a", "c"],
      ["b"],
      ["d"],
    ]);
  });

  it("allows whole-game notes and validates timestamps against game duration", () => {
    expect(parseTime("", 1200)).toBeUndefined();
    expect(parseTime("18:42", 1200)).toBe(1122);
    expect(() => parseTime("18:79", 1200)).toThrow("分:秒");
    expect(() => parseTime("20:01", 1200)).toThrow("时长");
  });
});
