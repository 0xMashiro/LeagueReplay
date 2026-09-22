import type { LiveUiState } from "../generated/live/LiveUiState";
const equal = (a: unknown, b: unknown) =>
  JSON.stringify(a) === JSON.stringify(b);
export type UiConflict = { field: string; mine: unknown; saved: unknown };
export function mergeUi(
  base: LiveUiState,
  mine: LiveUiState,
  saved: LiveUiState,
  prefer: "mine" | "saved" = "mine",
) {
  const conflicts: UiConflict[] = [];
  const choose = <T>(
    field: string,
    a: T | undefined,
    b: T | undefined,
    c: T | undefined,
  ): T | undefined => {
    if (equal(b, a)) return c;
    if (equal(c, a) || equal(b, c)) return b;
    conflicts.push({ field, mine: b, saved: c });
    return prefer === "mine" ? b : c;
  };
  const collection = <T>(
    field: string,
    a: T[],
    b: T[],
    c: T[],
    key: (v: T) => string,
  ): T[] => {
    const [aa, bb, cc] = [a, b, c].map(
      (values) => new Map(values.map((v) => [key(v), v])),
    );
    return [...new Set([...cc.keys(), ...bb.keys(), ...aa.keys()])].flatMap(
      (id) => {
        const value = choose(
          `${field}:${id}`,
          aa.get(id),
          bb.get(id),
          cc.get(id),
        );
        return value === undefined ? [] : [value];
      },
    );
  };
  let players = collection(
    "players",
    base.players,
    mine.players,
    saved.players,
    (p) => p.id,
  );
  const bindings = players.flatMap((p) => p.accountIds);
  if (new Set(bindings).size !== bindings.length) {
    conflicts.push({
      field: "players",
      mine: mine.players,
      saved: saved.players,
    });
    players = prefer === "mine" ? mine.players : saved.players;
  }
  const ui: LiveUiState = {
    ...saved,
    myAccounts: collection(
      "myAccounts",
      base.myAccounts ?? [],
      mine.myAccounts ?? [],
      saved.myAccounts ?? [],
      (s) => s.account.id,
    ),
    notes: collection(
      "notes",
      base.notes,
      mine.notes,
      saved.notes,
      (n) => n.id,
    ),
    players,
    identityLinks: collection(
      "identityLinks",
      base.identityLinks ?? [],
      mine.identityLinks ?? [],
      saved.identityLinks ?? [],
      (link) => link.id,
    ),
    reviewed: collection(
      "reviewed",
      base.reviewed,
      mine.reviewed,
      saved.reviewed,
      (v) => v,
    ),
    premades: collection(
      "premades",
      base.premades,
      mine.premades,
      saved.premades,
      (p) => `${p.matchId}:${p.accountId}`,
    ),
    theme: choose("theme", base.theme, mine.theme, saved.theme) ?? null,
  };
  const linkedPlayers = new Set(ui.players.map((p) => p.id));
  const invalidLinks = ui.identityLinks.some(
    (a, index) =>
      !linkedPlayers.has(a.playerId) ||
      ui.identityLinks
        .slice(index + 1)
        .some(
          (b) =>
            a.accountId === b.accountId &&
            (a.matchId && b.matchId
              ? a.matchId === b.matchId
              : !a.matchId &&
                !b.matchId &&
                a.from! <= b.to! &&
                b.from! <= a.to!),
        ),
  );
  if (invalidLinks) {
    conflicts.push({
      field: "identityLinks",
      mine: mine.identityLinks,
      saved: saved.identityLinks,
    });
    const preferred = prefer === "mine" ? mine : saved;
    ui.identityLinks = preferred.identityLinks;
    ui.players = preferred.players;
  }
  return { ui, conflicts };
}
