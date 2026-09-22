import type { LiveUiState } from "../generated/live/LiveUiState";
export function playerForAccount(
  ui: LiveUiState,
  accountId: string,
  matchId: string,
  at: number,
) {
  const links = (ui.identityLinks ?? []).filter(
    (link) => link.accountId === accountId,
  );
  const override =
    links.find((link) => link.matchId === matchId) ??
    links.find(
      (link) =>
        !link.matchId &&
        link.from !== null &&
        link.to !== null &&
        at >= link.from &&
        at <= link.to,
    );
  return override
    ? ui.players.find((p) => p.id === override.playerId)
    : ui.players.find((p) => p.accountIds.includes(accountId));
}
