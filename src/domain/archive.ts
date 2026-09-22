import { ui } from "../i18n";
import type { MatchEvent } from "./types";

// Keep separate player lanes and bound each group to its first purchase.
// A chain of purchases cannot accidentally absorb minutes of play.
export function groupPurchases(events: MatchEvent[]): MatchEvent[][] {
  const groups: MatchEvent[][] = [];
  const lastByPlayer = new Map<number, MatchEvent[]>();
  for (const event of events
    .filter((event) =>
      ["purchase", "sale", "undo", "destroy"].includes(event.kind),
    )
    .toSorted((a, b) => a.at - b.at)) {
    const previous = lastByPlayer.get(event.participantId);
    if (
      previous &&
      previous[0].kind === event.kind &&
      event.at - previous[0].at <= 20
    )
      previous.push(event);
    else {
      const group = [event];
      groups.push(group);
      lastByPlayer.set(event.participantId, group);
    }
  }
  return groups;
}

export function formatTime(seconds: number): string {
  return `${Math.floor(seconds / 60)
    .toString()
    .padStart(2, "0")}:${Math.floor(seconds % 60)
    .toString()
    .padStart(2, "0")}`;
}

export function parseTime(text: string, duration: number): number | undefined {
  if (!text.trim()) return undefined;
  const parts = /^(\d{1,3}):([0-5]\d)$/.exec(text.trim());
  if (!parts) throw new Error(ui("请输入分:秒，例如 18:42"));
  const seconds = Number(parts[1]) * 60 + Number(parts[2]);
  if (seconds > duration) throw new Error(ui("时间不能超过本局时长"));
  return seconds;
}
