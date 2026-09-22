import type { PlayPage } from "./generated/live/PlayPage";
import type { PlayQuery } from "./generated/live/PlayQuery";
import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import { ipcError } from "./ipc-error";
import type { LiveWorkspace } from "./generated/live/LiveWorkspace";
import type { WorkspaceUpdate } from "./generated/live/WorkspaceUpdate";
import type { LiveUiState } from "./generated/live/LiveUiState";
import type { Region } from "./generated/library/Region";
import type { SearchResult } from "./generated/library/SearchResult";
import type { ReviewGame } from "./generated/library/ReviewGame";
import type { LibraryEntry } from "./generated/library/LibraryEntry";
import type { FollowingState } from "./generated/library/FollowingState";
import type { FeedEntry } from "./generated/library/FeedEntry";
import type { ReplayJob } from "./generated/library/ReplayJob";
import type { BackupSelection } from "./generated/library/BackupSelection";
import type { RestoreBackupRequest } from "./generated/library/RestoreBackupRequest";
import type { Settings } from "./generated/desktop/Settings";
import type { Preferences } from "./generated/desktop/Preferences";
import type { ResourceData } from "./generated/resources/ResourceData";

type Contract<Args, Result> = { args: Args; result: Result };
type Id = { id: string };

/** Application commands only; plugin event APIs keep their own Tauri contracts. */
export interface Commands {
  play_page: Contract<{ query: PlayQuery }, PlayPage>;
  live_workspace: Contract<undefined, LiveWorkspace>;
  sync_workspace: Contract<{ cursor?: number | null }, WorkspaceUpdate>;
  save_live_ui: Contract<{ ui: LiveUiState; revision: number }, number>;
  end_live_session: Contract<{ sessionId: string }, void>;
  retract_manual_game: Contract<{ observationId: string }, void>;
  move_live_matches: Contract<
    { observationIds: string[]; destinationId?: string | null; title: string },
    string
  >;
  rename_live_session: Contract<{ sessionId: string; title: string }, void>;
  merge_live_sessions: Contract<
    { sourceId: string; destinationId: string },
    void
  >;
  split_live_session: Contract<
    { sessionId: string; segmentId: string; title: string },
    void
  >;
  query_regions: Contract<undefined, Region[]>;
  search_player: Contract<{ server: string; riotId: string }, SearchResult>;
  search_current_player: Contract<undefined, SearchResult>;
  player_history_page: Contract<
    { server: string; puuid: string; start: number },
    SearchResult
  >;
  open_review_game: Contract<
    { server: string; gameId: string; puuid: string; refresh: boolean },
    ReviewGame
  >;
  saved_review_game: Contract<Id & { puuid?: string | null }, ReviewGame>;
  ensure_review_timeline: Contract<Id & { puuid: string }, ReviewGame>;
  list_library: Contract<
    { start: number; filter: string; query?: string | null },
    LibraryEntry[]
  >;
  bookmark_review_game: Contract<Id & { bookmarked: boolean }, void>;
  backfill_review_game: Contract<
    Id & { puuid: string; sessionId?: string | null },
    void
  >;
  backfill_games: Contract<
    {
      server: string;
      gameIds: string[];
      puuid: string;
      sessionId?: string | null;
      title: string;
    },
    void
  >;
  following_state: Contract<undefined, FollowingState>;
  follow_player: Contract<{ server: string; puuid: string }, string>;
  edit_following: Contract<Id & { label: string; paused: boolean }, void>;
  unfollow_player: Contract<Id, void>;
  sync_following: Contract<{ id?: string | null }, void>;
  following_feed: Contract<
    { id?: string | null; unread: boolean; start: number },
    FeedEntry[]
  >;
  fill_following_history: Contract<Id, number>;
  replay_jobs: Contract<undefined, ReplayJob[]>;
  download_replay: Contract<Id, void>;
  open_replay: Contract<Id, void>;
  reveal_replay: Contract<Id, void>;
  cancel_replay: Contract<Id, void>;
  remove_replay: Contract<Id, void>;
  replay_directory: Contract<undefined, string>;
  choose_replay_directory: Contract<undefined, string | null>;
  import_replay: Contract<Id, boolean>;
  seek_replay: Contract<Id & { at: number }, void>;
  export_backup: Contract<undefined, string | null>;
  export_backup_package: Contract<undefined, string | null>;
  choose_backup: Contract<undefined, BackupSelection | null>;
  restore_backup: Contract<{ request: RestoreBackupRequest }, string>;
  share_replay: Contract<Id, string | null>;
  desktop_settings: Contract<undefined, Settings>;
  save_desktop_settings: Contract<{ preferences: Preferences }, void>;
  set_autostart: Contract<{ enabled: boolean }, void>;
  desktop_ready: Contract<undefined, void>;
  exit_desktop: Contract<undefined, void>;
  cached_resources: Contract<undefined, ResourceData | null>;
  update_resources: Contract<undefined, ResourceData>;
}

export type Command = keyof Commands;
export type Args<C extends Command> = Commands[C]["args"];
export type Result<C extends Command> = Commands[C]["result"];

export type Call<C extends Command = Command> = {
  [K in C]: Args<K> extends undefined
    ? [command: K, args?: undefined]
    : [command: K, args: Args<K>];
}[C];

export function invoke<const T extends Call>(
  ...call: T
): Promise<Result<T[0]>> {
  return tauriInvoke<Result<T[0]>>(call[0], call[1]).catch(
    (reason: unknown) => {
      throw ipcError(call[0], reason);
    },
  );
}
