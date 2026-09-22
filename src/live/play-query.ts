import type { LiveWorkspace } from "../generated/live/LiveWorkspace";
import type { ObservedMatch } from "../generated/live/ObservedMatch";
import type { Match } from "../domain/types";
import type { Language } from "../i18n";
import type { ResourceData } from "../resources";
import { adaptMatch, participantAccountKey } from "./adapter";
import { playerForAccount } from "./identity";

export const initialPlayFilters = {
  query: "",
  account: "all",
  result: "all",
  source: "all",
  from: "",
  to: "",
};
export type PlayFilter = typeof initialPlayFilters;

export function indexPlay(
  view: Pick<LiveWorkspace, "accounts" | "matches" | "sessions" | "ui">,
  language: Language,
  resources: ResourceData,
) {
  const accountById = Object.fromEntries(view.accounts.map((a) => [a.id, a]));
  const projectedMatches = new Map<string, Match | undefined>();
  const archiveIds = new Set(view.matches.map((r) => r.id));
  const accountIds = new Set(view.matches.map((r) => r.accountId));
  const settled = new Map<string, Match>();
  const segmentTitles = new Map(
    view.sessions.flatMap((s) =>
      s.segments.map((segment) => [segment.id, s.title] as const),
    ),
  );
  const notes = new Map<string, string[]>();
  let noteCount = 0;
  for (const note of view.ui.notes) {
    const entries = notes.get(note.matchId) ?? [];
    entries.push(`${note.body} ${note.tags.join(" ")}`);
    notes.set(note.matchId, entries);
    if (archiveIds.has(note.matchId)) noteCount++;
  }
  const premades = new Map<string, Set<string>>();
  for (const mark of view.ui.premades) {
    const accounts = premades.get(mark.matchId) ?? new Set<string>();
    accounts.add(mark.accountId);
    premades.set(mark.matchId, accounts);
  }
  const championAliases = new Map(
    Object.entries(resources.catalog.champions).map(([alias, value]) => [
      value.key,
      alias,
    ]),
  );
  const rows = view.matches.map((record) => {
    const own = accountById[record.accountId];
    const projected = adaptMatch(record, own.puuid);
    projectedMatches.set(record.observationId, projected);
    if (projected) settled.set(projected.id, projected);
    const started = projected
      ? Date.parse(projected.startedAt)
      : record.firstSeen;
    const alias =
      projected?.champion ??
      championAliases.get(record.championId) ??
      String(record.championId);
    const champion =
      resources.names[language]?.champions[alias] ??
      resources.catalog.champions[alias]?.title ??
      alias;
    const text = [
      record.id,
      record.queueName,
      own.riotId,
      segmentTitles.get(record.segmentId),
      champion,
      ...(notes.get(record.id) ?? []),
    ]
      .join(" ")
      .toLocaleLowerCase(language);
    const players = new Set<string>(),
      groupedPlayers = new Set<string>();
    const people = record.game?.participants ?? [];
    const mine = people.find((p) => p.puuid === own.puuid);
    if (mine?.team)
      for (const person of people) {
        const key = participantAccountKey(record.platform, person);
        if (!key || person.id === mine.id || person.team !== mine.team)
          continue;
        const player = playerForAccount(view.ui, key, record.id, started);
        if (!player) continue;
        players.add(player.id);
        if (premades.get(record.id)?.has(key)) groupedPlayers.add(player.id);
      }
    return { record, started, projected, text, players, groupedPlayers };
  });
  return {
    rows,
    sessions: view.sessions,
    language,
    accountById,
    accountIds,
    projectedMatches,
    archiveIds,
    settled: [...settled.values()],
    wins: [...settled.values()].filter((m) => m.win).length,
    noteCount,
  };
}

export function filterPlay(
  index: ReturnType<typeof indexPlay>,
  filters: PlayFilter,
  player: string,
  onlyPremade: boolean,
  scope: string,
) {
  const query = filters.query.trim().toLocaleLowerCase(index.language);
  const from = filters.from
    ? Date.parse(`${filters.from}T00:00:00`)
    : -Infinity;
  const to = filters.to ? Date.parse(`${filters.to}T23:59:59.999`) : Infinity;
  const recordsBySegment = new Map<string, ObservedMatch[]>();
  for (const row of index.rows) {
    const { record, projected, started } = row;
    if (filters.account !== "all" && filters.account !== record.accountId)
      continue;
    if (
      filters.result !== "all" &&
      (!projected || (filters.result === "win") !== projected.win)
    )
      continue;
    if (
      (filters.source === "automatic" && !record.automatic) ||
      (filters.source === "manual" && !record.manual)
    )
      continue;
    if (started < from || started > to || !row.text.includes(query)) continue;
    if (
      player !== "all" &&
      !(onlyPremade ? row.groupedPlayers : row.players).has(player)
    )
      continue;
    const records = recordsBySegment.get(record.segmentId) ?? [];
    records.push(record);
    recordsBySegment.set(record.segmentId, records);
  }
  const sessions = index.sessions.filter(
    (s) =>
      (scope === "all" || (scope === "current") === (s.endedAt === null)) &&
      (s.endedAt === null ||
        s.segments.some((segment) => recordsBySegment.has(segment.id))),
  );
  return { recordsBySegment, sessions };
}
