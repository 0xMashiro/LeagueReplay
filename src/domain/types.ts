export interface Account {
  id: string;
  riotId: string;
  platform: string;
  region: string;
  champion: string;
  kind: "local" | "followed";
  label: string;
}

export interface Participant {
  puuid?: string;
  id: number;
  team: number;
  placement?: number;
  name: string;
  champion: string;
  role: string;
  kills: number;
  deaths: number;
  assists: number;
  cs: number;
  gold: number;
  damage: number;
  items: number[];
}

export interface MatchEvent {
  id: string;
  at: number;
  participantId: number;
  kind:
    | "purchase"
    | "sale"
    | "undo"
    | "destroy"
    | "kill"
    | "multikill"
    | "objective";
  itemId?: number;
  restoredItemId?: number;
  label: string;
}

export interface Match {
  id: string; // Platform-qualified game identity, shared by all views.
  accountId: string;
  startedAt: string;
  duration: number;
  win: boolean;
  champion: string;
  queue: string;
  patch: string;
  participantId: number;
  participants: Participant[];
  multikills?: {
    double: number;
    triple: number;
    quadra: number;
    penta: number;
  };
  events: MatchEvent[];
  timelineAvailable: boolean;
  replay: "available" | "expired" | "unavailable";
  source: "observed" | "followed" | "search";
}

export interface Note {
  id: string;
  matchId: string;
  body: string;
  at?: number;
  participantId?: number;
  tags: string[];
  updatedAt: string;
}

export interface ArchiveState {
  notes: Note[];
  reviewed: string[];
}

export type Page =
  | "play"
  | "following"
  | "search"
  | "library"
  | "settings";
