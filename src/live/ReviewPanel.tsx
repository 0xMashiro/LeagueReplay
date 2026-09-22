import { useState, useEffect } from "react";
import { invoke } from "../ipc";
import {
  Button,
  Menu,
  MenuItem,
  MenuList,
  MenuPopover,
  MenuTrigger,
  MessageBar,
  MessageBarActions,
  ProgressBar,
  Tooltip,
} from "@fluentui/react-components";
import {
  ArrowDownload20Regular,
  Bookmark20Filled,
  Bookmark20Regular,
  MoreHorizontal20Regular,
  Play20Regular,
  Dismiss20Regular,
} from "@fluentui/react-icons";
import type { ReviewGame } from "../generated/library/ReviewGame";
import type { LiveStore } from "./useLiveWorkspace";
import type { ArchiveState } from "../domain/types";
import { ArchiveContext } from "../archive-context";
import { MatchDetail } from "../pages/MatchDetail";
import { NotesPanel } from "../components/NotesPanel";
import { useI18n } from "../i18n";
import { adaptReview } from "./adapter";
import { BackfillDialog } from "./BackfillDialog";
import { useReplayJobs } from "./useReplayJobs";
import { RemoveReplayDialog } from "./ReplayTasks";

export interface ReviewNavigation {
  onDirtyChange: (dirty: boolean) => void;
  onNavigate: (action: () => void) => void;
}
export function ReviewPanel({
  game,
  onChange,
  onBack,
  store,
  onDirtyChange,
  onNavigate,
}: ReviewNavigation & {
  game: ReviewGame;
  onChange: (game: ReviewGame) => void;
  onBack: () => void;
  store: LiveStore;
}) {
  const { t, error: explain } = useI18n();
  const { jobs, error: pollError } = useReplayJobs();
  const [timelineAttempt, setTimelineAttempt] = useState(0);
  const [timelineLoading, setTimelineLoading] = useState(false);
  const [timelineFailure, setTimelineFailure] = useState<unknown>(null);
  useEffect(() => {
    if (game.timeline || store.workspace?.status.state !== "connected") return;
    let disposed = false;
    setTimelineLoading(true);
    setTimelineFailure(null);
    void invoke("ensure_review_timeline", {
      id: game.id,
      puuid: game.account.puuid,
    })
      .then((fresh) => {
        if (!disposed) onChange(fresh);
      })
      .catch((reason: unknown) => {
        if (!disposed) setTimelineFailure(reason);
      })
      .finally(() => {
        if (!disposed) setTimelineLoading(false);
      });
    return () => {
      disposed = true;
    };
  }, [
    game.id,
    game.account.puuid,
    game.timeline,
    onChange,
    store.workspace?.status.state,
    timelineAttempt,
  ]);
  const [error, setError] = useState("");
  const [message, setMessage] = useState("");
  const [dismissed, setDismissed] = useState("");
  const [busy, setBusy] = useState(false);
  const [backfill, setBackfill] = useState(false);
  const [removing, setRemoving] = useState(false);
  const job = jobs.find((j) => j.id === game.id);
  const downloading =
    job &&
    ["waiting", "queued", "downloading", "validating", "cancelling"].includes(
      job.state,
    );
  const ready = job?.state === "ready";
  const match = adaptReview(game);
  const view = store.workspace!;
  const recorded = view.matches.find(
    (m) => m.id === game.id && m.accountId === game.account.id,
  );
  const state: ArchiveState = {
    notes: view.ui.notes,
    reviewed: view.ui.reviewed,
  };
  const perform = async (action: () => Promise<void>) => {
    setBusy(true);
    setError("");
    setMessage("");
    setDismissed("");
    try {
      await action();
    } catch (reason) {
      setError(String(reason));
    } finally {
      setBusy(false);
    }
  };
  const bookmark = () =>
    perform(async () => {
      await invoke("bookmark_review_game", {
        id: game.id,
        bookmarked: !game.bookmarked,
      });
      onChange({ ...game, bookmarked: !game.bookmarked });
    });
  const replay = () =>
    perform(async () => {
      await invoke(ready ? "open_replay" : "download_replay", { id: game.id });
      if (ready) setMessage(t("opened"));
    });
  const refresh = () =>
    void perform(async () => {
      const fresh = await invoke("open_review_game", {
        server: game.server,
        gameId: game.gameId,
        puuid: game.account.puuid,
        refresh: true,
      });
      onChange(fresh);
    });
  const actions = (
    <>
      <Tooltip
        content={game.bookmarked ? t("unsave") : t("save")}
        relationship="label"
      >
        <Button
          aria-label={game.bookmarked ? t("unsave") : t("save")}
          appearance="subtle"
          icon={game.bookmarked ? <Bookmark20Filled /> : <Bookmark20Regular />}
          disabled={busy}
          onClick={() => void bookmark()}
        />
      </Tooltip>
      <Button
        appearance="primary"
        icon={ready ? <Play20Regular /> : <ArrowDownload20Regular />}
        disabled={busy || !!downloading}
        onClick={() => void replay()}
      >
        {ready
          ? t("open")
          : downloading
            ? t(
                job.state === "waiting"
                  ? "waitingClient"
                  : job.state === "queued"
                    ? "queued"
                    : job.state === "validating"
                      ? "validating"
                      : job.state === "cancelling"
                        ? "cancelling"
                        : "downloading",
              )
            : t("download")}
      </Button>
      <Menu>
        <MenuTrigger disableButtonEnhancement>
          <Button
            appearance="subtle"
            icon={<MoreHorizontal20Regular />}
            aria-label={t("more")}
          />
        </MenuTrigger>
        <MenuPopover>
          <MenuList>
            <MenuItem
              disabled={busy || !!downloading}
              onClick={() =>
                void perform(async () => {
                  if (await invoke("import_replay", { id: game.id }))
                    setMessage(t("importReplayDone"));
                })
              }
            >
              {t("importReplay")}
            </MenuItem>
            <MenuItem
              disabled={busy || !ready || store.hasPending}
              onClick={() =>
                void perform(async () => {
                  const path = await invoke("share_replay", {
                    id: game.id,
                  });
                  if (path) setMessage(t("packageSaved", { path }));
                })
              }
            >
              {t("shareReplay")}
            </MenuItem>
            {job &&
              ["waiting", "queued", "downloading"].includes(job.state) && (
                <MenuItem
                  disabled={busy}
                  onClick={() =>
                    void perform(async () => {
                      await invoke("cancel_replay", { id: game.id });
                    })
                  }
                >
                  {t("cancelDownload")}
                </MenuItem>
              )}
            {job && ["ready", "missing"].includes(job.state) && (
              <MenuItem disabled={busy} onClick={() => setRemoving(true)}>
                {t("removeReplay")}
              </MenuItem>
            )}
            <MenuItem
              onClick={() =>
                store.update((ui) => ({
                  ...ui,
                  reviewed: ui.reviewed.includes(game.id)
                    ? ui.reviewed.filter((id) => id !== game.id)
                    : [...ui.reviewed, game.id],
                }))
              }
            >
              {t(
                view.ui.reviewed.includes(game.id)
                  ? "reviewed"
                  : "markReviewed",
              )}
            </MenuItem>
            <MenuItem
              disabled={!ready || busy}
              onClick={() =>
                void perform(async () => {
                  await invoke("reveal_replay", { id: game.id });
                })
              }
            >
              {t("reveal")}
            </MenuItem>
            <MenuItem disabled={busy} onClick={refresh}>
              {t("refresh")}
            </MenuItem>
            <MenuItem
              disabled={busy || !!recorded}
              onClick={() => setBackfill(true)}
            >
              {recorded
                ? t(recorded.manual ? "imported" : "recorded")
                : t("backfill")}
            </MenuItem>
          </MenuList>
        </MenuPopover>
      </Menu>
    </>
  );
  const failure = error || job?.error || pollError;
  return (
    <ArchiveContext.Provider
      value={{
        state,
        accounts: {
          [game.account.id]: {
            ...game.account,
            region: game.account.platform,
            kind: "local",
            champion: "",
            label: game.account.riotId,
          },
        },
        saving: store.saving || store.hasPending,
        error: store.conflict ? "ui.conflict" : store.error,
        retry: () => void store.retry(),
        update: (change) =>
          store.update((ui) => {
            const next = change({
              ...state,
              notes: ui.notes,
              reviewed: ui.reviewed,
            });
            return { ...ui, notes: next.notes, reviewed: next.reviewed };
          }),
      }}
    >
      <div className="review-workspace">
        {removing && (
          <RemoveReplayDialog
            id={game.id}
            onClose={() => setRemoving(false)}
            onRemoved={() => setMessage(t("removedReplay"))}
          />
        )}
        {(failure ||
          message ||
          downloading ||
          job?.state === "missing" ||
          game.timeline === null) && (
          <div className="review-feedback">
            {failure && failure !== dismissed && (
              <MessageBar
                intent={
                  ["replay.versionMismatch", "replay.clientPreparing"].includes(
                    failure,
                  )
                    ? "warning"
                    : "error"
                }
              >
                {explain(failure)}
                <MessageBarActions
                  containerAction={
                    <Button
                      appearance="transparent"
                      icon={<Dismiss20Regular />}
                      aria-label={t("dismiss")}
                      onClick={() => setDismissed(failure)}
                    />
                  }
                />
              </MessageBar>
            )}
            {message && <MessageBar intent="success">{message}</MessageBar>}
            {job?.state === "missing" && (
              <MessageBar>{t("missing")}</MessageBar>
            )}
            {game.timeline === null && (
              <MessageBar>
                {t("timelineMissing")}
                {(timelineFailure || game.timelineError) && (
                  <> {explain(timelineFailure || game.timelineError)}</>
                )}
                <MessageBarActions>
                  <Button
                    disabled={timelineLoading || store.workspace?.status.state !== "connected"}
                    onClick={() => setTimelineAttempt((attempt) => attempt + 1)}
                  >
                    {t("retry")}
                  </Button>
                </MessageBarActions>
              </MessageBar>
            )}
            {downloading && (
              <div className="replay-progress" role="status">
                <span>
                  {t(
                    job.state === "waiting"
                      ? "waitingClient"
                      : job.state === "queued"
                        ? "queued"
                        : job.state === "validating"
                          ? "validating"
                          : job.state === "cancelling"
                            ? "cancelling"
                            : "downloading",
                  )}{" "}
                  · {(job.received / 1048576).toFixed(1)} MB
                  {job.total ? ` / ${(job.total / 1048576).toFixed(1)} MB` : ""}
                </span>
                <ProgressBar
                  aria-label={t("downloading")}
                  value={job.total ? job.received / job.total : undefined}
                />
              </div>
            )}
          </div>
        )}
        {match ? (
          <MatchDetail
            match={match}
            onBack={() => onNavigate(onBack)}
            onDownload={() => {}}
            onDirtyChange={onDirtyChange}
            actions={actions}
            onSeek={(at) =>
              void perform(async () => {
                await invoke("seek_replay", { id: game.id, at });
              })
            }
          />
        ) : (
          <div className="page-scroll live-page">
            <div className="page-toolbar">
              <Button onClick={() => onNavigate(onBack)}>{t("back")}</Button>
              <div className="detail-primary">{actions}</div>
            </div>
            <MessageBar>{t("unsupportedMode")}</MessageBar>
            <NotesPanel
              match={{
                id: game.id,
                duration: game.game.duration,
                participants: [],
              }}
              onDirtyChange={onDirtyChange}
            />
          </div>
        )}
        {backfill && (
          <BackfillDialog
            game={game}
            store={store}
            onClose={() => setBackfill(false)}
          />
        )}
      </div>
    </ArchiveContext.Provider>
  );
}
