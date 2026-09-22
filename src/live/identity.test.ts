import { expect, it } from "vitest";
import { playerForAccount } from "./identity";
import type { LiveUiState } from "../generated/live/LiveUiState";
it("applies match, date range and account identity in that order", () => {
  const state: LiveUiState = {
    myAccounts: [],
    notes: [],
    reviewed: [],
    premades: [],
    theme: null,
    players: [
      { id: "owner", name: "Owner", accountIds: ["HN1:a"] },
      { id: "guest", name: "Guest", accountIds: [] },
      { id: "friend", name: "Friend", accountIds: [] },
    ],
    identityLinks: [
      {
        id: "period",
        accountId: "HN1:a",
        playerId: "guest",
        from: 100,
        to: 200,
        matchId: null,
      },
      {
        id: "game",
        accountId: "HN1:a",
        playerId: "friend",
        from: null,
        to: null,
        matchId: "HN1_42",
      },
    ],
  };
  expect(playerForAccount(state, "HN1:a", "HN1_1", 99)?.id).toBe("owner");
  expect(playerForAccount(state, "HN1:a", "HN1_1", 100)?.id).toBe("guest");
  expect(playerForAccount(state, "HN1:a", "HN1_42", 150)?.id).toBe("friend");
  expect(playerForAccount(state, "HN1:a", "HN1_1", 201)?.id).toBe("owner");
  expect(playerForAccount(state, "HN10:a", "HN1_1", 150)).toBeUndefined();
});
