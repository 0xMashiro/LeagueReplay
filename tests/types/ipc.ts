// Compiled by tsc, never executed. @ts-expect-error also fails if a boundary is widened.
import { invoke } from "../../src/ipc";
import type { LiveStore } from "../../src/live/useLiveWorkspace";
import type { ReviewGame } from "../../src/generated/library/ReviewGame";

export function checkIpcContract(store: LiveStore) {
  const review: Promise<ReviewGame> = invoke("saved_review_game", {
    id: "HN1_42",
  });
  void review;
  void invoke("search_current_player");
  void invoke("player_history_page", {
    server: "TENCENT_HN1",
    puuid: "player",
    start: 0,
  });
  void store.command(
    [
      "restore_backup",
      {
        request: {
          path: "backup.jsonl",
          fingerprint: "current",
          sourceFingerprint: "incoming",
          merge: true,
        },
      },
    ],
    (path) => {
      const text: string = path;
      void text;
    },
  );

  // @ts-expect-error Unknown command names must not reach native IPC.
  void invoke("serach_player", { server: "HN1", riotId: "A#TEST" });
  // @ts-expect-error Query identity must include a region.
  void invoke("player_history_page", { puuid: "player", start: 0 });
  // @ts-expect-error Match IDs are strings.
  void invoke("download_replay", { id: 42 });
  // @ts-expect-error Callers cannot claim an arbitrary response type.
  const wrong: Promise<number> = invoke("saved_review_game", { id: "HN1_42" });
  void wrong;
  // @ts-expect-error A split requires a segment boundary even through the workspace hook.
  void store.command([
    "split_live_session",
    { sessionId: "session", title: "split" },
  ]);
  // @ts-expect-error Command/argument alternatives must remain correlated.
  void invoke(Math.random() > 0.5 ? "search_player" : "player_history_page", {
    server: "HN1",
    riotId: "A#TEST",
  });
}
