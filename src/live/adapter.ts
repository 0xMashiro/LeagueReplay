import { ui, translate } from "../i18n";
import { gameLabel, queueLabel } from "../game-labels";
import type { ReviewGame } from "../generated/library/ReviewGame";
import type { Match } from "../domain/types";
import type { ObservedMatch } from "../generated/live/ObservedMatch";
import { resourceData, type ResourceData } from "../resources";

let aliasCatalog: ResourceData["catalog"] | undefined;
let championAliases = new Map<number, string>();
export function championAlias(id: number) {
  const catalog = resourceData().catalog;
  if (catalog !== aliasCatalog) {
    championAliases = new Map(
      Object.entries(catalog.champions).map(([alias, champion]) => [
        champion.key,
        alias,
      ]),
    );
    aliasCatalog = catalog;
  }
  return championAliases.get(id) ?? String(id);
}
export const participantAccountKey = (
  platform: string,
  participant: { puuid: string | null } | undefined,
) => (participant?.puuid ? `${platform}:${participant.puuid}` : undefined);
export function knownPlayerNames(
  records: ObservedMatch[],
): Record<string, string> {
  const names: Record<string, string> = {};
  for (const record of records)
    for (const participant of record.game?.participants ?? []) {
      const key = participantAccountKey(record.platform, participant);
      if (key)
        names[key] =
          `${participant.name || ui("未知玩家")} · ${record.platform}`;
    }
  return names;
}
export function adaptMatch(
  record: ObservedMatch,
  puuid: string,
): Match | undefined {
  if (record.state !== "ready" || !record.game) return undefined;
  return projectMatch(
    record.id,
    record.accountId,
    record.game,
    puuid,
    record.queueName,
    record.firstSeen,
  );
}

function projectMatch(
  id: string,
  accountId: string,
  game: ReviewGame["game"],
  puuid: string,
  queueName: string,
  firstSeen = 0,
): Match | undefined {
  const me = game?.participants.find((p) => p.puuid === puuid);
  if (!me || me.win === null) return undefined;
  return {
    id,
    accountId,
    startedAt: new Date(game.startedAt || firstSeen).toISOString(),
    duration: game.duration,
    win: me.win,
    champion: championAlias(me.championId),
    queue: queueLabel(game.queueId, queueName),
    patch: game.version,
    participantId: me.id,
    multikills: {
      double: me.doubleKills,
      triple: me.tripleKills,
      quadra: me.quadraKills,
      penta: me.pentaKills,
    },
    participants: game.participants.map((p) => ({
      id: p.id,
      puuid: p.puuid ?? undefined,
      name: p.name || ui("未知玩家"),
      team: p.team,
      placement: p.placement ?? undefined,
      champion: championAlias(p.championId),
      role: gameLabel(p.role || "位置未知"),
      kills: p.kills,
      deaths: p.deaths,
      assists: p.assists,
      cs: p.cs,
      gold: p.gold,
      damage: p.damage,
      items: p.items,
    })),
    events: [],
    timelineAvailable: false,
    replay: "unavailable",
    source: "observed",
  };
}

export function adaptReview(review: ReviewGame): Match | undefined {
  const match = projectMatch(
    review.id,
    review.account.id,
    review.game,
    review.account.puuid,
    review.queueName,
  );
  if (!match) return undefined;
  return {
    ...match,
    source: "search",
    timelineAvailable: review.timeline !== null,
    events: (review.timeline ?? []).map((event) => ({
      id: event.id,
      at: event.at,
      participantId: event.participantId,
      kind: event.kind,
      itemId: event.itemId ?? undefined,
      restoredItemId: event.restoredItemId ?? undefined,
      label:
        event.label === "purchase"
          ? ui("购买装备")
          : event.label === "sale"
            ? translate("itemSold")
            : event.label === "undoSale"
              ? translate("itemUndoSale")
              : event.label === "undoPurchase"
                ? translate("itemUndoPurchase")
                : event.label === "undo"
                  ? translate("itemUndo")
                  : event.label === "destroy"
                    ? translate("itemConsumed")
                    : event.label === "kill"
                      ? ui("击杀英雄")
                      : gameLabel(event.monster) || ui("摧毁建筑"),
    })),
  };
}

export function withPlayer(
  record: ObservedMatch,
  ownPuuid: string,
  accounts: string[],
  premades: { matchId: string; accountId: string }[],
  onlyPremade: boolean,
) {
  const people = record.game?.participants ?? [];
  const me = people.find((p) => p.puuid === ownPuuid);
  if (!me?.team) return false;
  return people.some((person) => {
    const key = participantAccountKey(record.platform, person);
    return (
      !!key &&
      person.id !== me.id &&
      person.team === me.team &&
      accounts.includes(key) &&
      (!onlyPremade ||
        premades.some(
          (mark) => mark.matchId === record.id && mark.accountId === key,
        ))
    );
  });
}
