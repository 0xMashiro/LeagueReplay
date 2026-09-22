import { useEffect, useState } from "react";
import { invoke } from "../ipc";
import {
  Button,
  Dialog,
  DialogActions,
  DialogBody,
  DialogContent,
  DialogSurface,
  DialogTitle,
  Dropdown,
  Field,
  Input,
  MessageBar,
  MenuItem,
  Option,
  Spinner,
  Switch,
  Tab,
  TabList,
} from "@fluentui/react-components";
import { Add20Regular, ArrowSync20Regular } from "@fluentui/react-icons";
import type { FeedEntry } from "../generated/library/FeedEntry";
import type { Subscription } from "../generated/library/Subscription";
import type { LiveStore } from "./useLiveWorkspace";
import { useFollowing } from "./useFollowing";
import { useI18n } from "../i18n";
import { championAlias } from "./adapter";
import { useReview } from "./useReview";
import { ReviewPanel, type ReviewNavigation } from "./ReviewPanel";
import { MatchRow } from "./MatchRow";
import { queueLabel } from "../game-labels";
import { useReplayActions } from "./useReplayActions";
import { useListReturn } from "./useListReturn";
import { PlayerHistoryLink } from "./PlayerHistoryLink";
import { PlayerProfiles } from "./PlayerProfiles";
import { AssociatePlayer } from "./AssociatePlayer";

export function FollowingPanel({
  store,
  onSearch,
  ...navigation
}: ReviewNavigation & { store: LiveStore; onSearch: () => void }) {
  const { t, date, error: explain } = useI18n();
  const following = useFollowing();
  const review = useReview();
  const list = useListReturn(!!review.game);
  const replay = useReplayActions();
  const [selected, setSelected] = useState("all");
  const [tab, setTab] = useState("feed");
  const [unread, setUnread] = useState(false);
  const [start, setStart] = useState(0);
  const [entries, setEntries] = useState<FeedEntry[]>([]);
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(false);
  const [attempt, setAttempt] = useState(0);
  const [editing, setEditing] = useState<Subscription>();
  const [label, setLabel] = useState("");
  const [paused, setPaused] = useState(false);
  const subscriptions = following.state.subscriptions;
  useEffect(() => {
    if (selected !== "all" && !subscriptions.some((s) => s.id === selected)) {
      setSelected("all");
      setStart(0);
    }
  }, [subscriptions, selected]);
  const selectedPlayer = subscriptions.find((s) => s.id === selected);
  const version = JSON.stringify(
    subscriptions.map((s) => [s.id, s.lastSynced, s.lastError]),
  );
  const reviewed = store.workspace!.ui.reviewed.join("|");
  useEffect(() => {
    setEntries([]);
  }, [selected, unread, start]);
  useEffect(() => {
    if (review.game) return;
    let disposed = false;
    setLoading(true);
    setError("");
    void invoke("following_feed", {
      id: selected === "all" ? null : selected,
      unread,
      start,
    })
      .then((rows) => {
        if (!disposed) setEntries(rows);
      })
      .catch((e) => {
        if (!disposed) setError(String(e));
      })
      .finally(() => {
        if (!disposed) setLoading(false);
      });
    return () => {
      disposed = true;
    };
  }, [selected, unread, start, version, reviewed, review.game, attempt]);
  const edit = (s: Subscription) => {
    setEditing(s);
    setLabel(s.label);
    setPaused(s.paused);
  };
  if (review.game)
    return (
      <ReviewPanel
        game={review.game}
        onChange={review.setGame}
        onBack={() => review.setGame(undefined)}
        store={store}
        {...navigation}
      />
    );
  return (
    <div className="page-scroll live-page following-page" ref={list.scrollRef}>
      <header className="live-heading">
        <div>
          <h1>{t("followTitle")}</h1>
        </div>
        <Button appearance="primary" icon={<Add20Regular />} onClick={onSearch}>
          {t("followAdd")}
        </Button>
      </header>
      <TabList
        className="following-tabs"
        selectedValue={tab}
        onTabSelect={(_, data) => setTab(String(data.value))}
      >
        <Tab value="feed">{t("followingFeed")}</Tab>
        <Tab value="players">{t("playerProfiles")}</Tab>
      </TabList>
      {tab === "players" ? (
        <PlayerProfiles
          store={store}
          accounts={subscriptions.map((s) => s.account)}
        />
      ) : (
        <>
          <div className="view-controls">
            <div className="page-toolbar following-toolbar">
              <Dropdown
                aria-label={t("followTitle")}
                value={
                  selectedPlayer
                    ? selectedPlayer.label || selectedPlayer.account.riotId
                    : t("followAll")
                }
                selectedOptions={[selected]}
                onOptionSelect={(_, data) => {
                  setSelected(data.optionValue!);
                  setStart(0);
                }}
              >
                <Option value="all">{t("followAll")}</Option>
                {subscriptions.map((s) => (
                  <Option
                    key={s.id}
                    value={s.id}
                    text={s.label || s.account.riotId}
                  >
                    {s.label || s.account.riotId} · {s.account.platform}
                  </Option>
                ))}
              </Dropdown>
              <Switch
                checked={unread}
                label={t("followUnread")}
                onChange={(_, d) => {
                  setUnread(d.checked);
                  setStart(0);
                }}
              />
              <Button
                icon={<ArrowSync20Regular />}
                disabled={
                  following.busy ||
                  following.state.syncing ||
                  (selectedPlayer
                    ? selectedPlayer.paused
                    : !subscriptions.some((s) => !s.paused))
                }
                onClick={() =>
                  void following.run("sync_following", {
                    id: selected === "all" ? null : selected,
                  })
                }
              >
                {t(
                  following.state.syncing || following.busy
                    ? "followSyncing"
                    : "followSync",
                )}
              </Button>
            </div>
          </div>
          {(following.error || error || review.error) && (
            <MessageBar intent="error">
              {explain(following.error || error || review.error)}
              {error && (
                <Button
                  appearance="transparent"
                  disabled={loading}
                  onClick={() => setAttempt((n) => n + 1)}
                >
                  {t("retry")}
                </Button>
              )}
            </MessageBar>
          )}
          {!!subscriptions.length && (
            <details
              className="follow-management"
              open={selected !== "all" ? true : undefined}
            >
              <summary>
                {t("manageFollowing", { count: subscriptions.length })}
                {subscriptions.some((s) => s.lastError || s.limited) && (
                  <small>{t("followAttention")}</small>
                )}
              </summary>
              <p className="muted query-hint">{t("followSchedule")}</p>
              <div className="follow-status-list">
                {subscriptions
                  .filter((s) => selected === "all" || s.id === selected)
                  .map((s) => (
                    <div className="follow-status" key={s.id}>
                      <div>
                        <strong>
                          {s.label || <PlayerHistoryLink target={s.account} />}
                        </strong>
                        <small className="muted">
                          {s.label && (
                            <>
                              <PlayerHistoryLink target={s.account} /> ·{" "}
                            </>
                          )}
                          {s.account.platform} ·{" "}
                          {s.paused
                            ? t("followPaused")
                            : s.lastSynced
                              ? t("followLast", { date: date(s.lastSynced) })
                              : t("followNever")}
                        </small>
                        {s.lastError && (
                          <small className="loss-text">
                            {explain(s.lastError)}
                          </small>
                        )}
                        {s.limited && (
                          <small className="muted">{t("followLimited")}</small>
                        )}
                      </div>
                      <AssociatePlayer account={s.account} store={store} />
                      <Button
                        disabled={
                          following.busy || following.state.syncing || s.paused
                        }
                        onClick={() =>
                          void following.run("fill_following_history", {
                            id: s.id,
                          })
                        }
                      >
                        {t("fillHistory")}
                      </Button>
                      <Button
                        appearance="subtle"
                        disabled={following.busy}
                        onClick={() => edit(s)}
                      >
                        {t("followEdit")}
                      </Button>
                    </div>
                  ))}
              </div>
            </details>
          )}
          {(loading || review.busy) && (
            <Spinner size="small" label={t("loading")} />
          )}
          {replay.error && (
            <MessageBar intent="error">{explain(replay.error)}</MessageBar>
          )}
          <div className="query-results" aria-busy={loading}>
            {entries.map(({ subscriptionId, game }) => {
              const hero = championAlias(game.championId);
              const done = store.workspace!.ui.reviewed.includes(game.id);
              return (
                <MatchRow
                  key={subscriptionId + ":" + game.id}
                  matchId={game.id}
                  rowKey={subscriptionId + ":" + game.id}
                  riotId={game.account.riotId}
                  summary={{
                    ...game,
                    champion: hero,
                    queue: queueLabel(game.queueId, ""),
                    items: game.items,
                  }}
                  identity={
                    subscriptions.find((s) => s.id === subscriptionId)?.label ||
                    game.account.riotId
                  }
                  provenance={t(done ? "reviewed" : "followPending")}
                  note={
                    store.workspace?.ui.notes.find((n) => n.matchId === game.id)
                      ?.body
                  }
                  job={replay.jobs.find((j) => j.id === game.id)}
                  disabled={loading || review.busy || replay.busy}
                  onOpen={() => {
                    list.remember(subscriptionId + ":" + game.id);
                    void review.open("saved_review_game", {
                      id: game.id,
                      puuid: game.account.puuid,
                    });
                  }}
                  onReplay={() => void replay.run(game.id)}
                  onReveal={() => void replay.reveal(game.id)}
                  menuItems={
                    <>
                      <MenuItem
                        onClick={() =>
                          void replay
                            .bookmark(game.id, !game.bookmarked)
                            .then((saved) => {
                              if (saved) setAttempt((n) => n + 1);
                            })
                        }
                      >
                        {t(game.bookmarked ? "unsave" : "save")}
                      </MenuItem>
                      <MenuItem
                        onClick={() =>
                          store.update((current) => ({
                            ...current,
                            reviewed: done
                              ? current.reviewed.filter((id) => id !== game.id)
                              : [...new Set([...current.reviewed, game.id])],
                          }))
                        }
                      >
                        {t(done ? "markUnreviewed" : "markReviewed")}
                      </MenuItem>
                    </>
                  }
                />
              );
            })}
          </div>
          {!loading && !error && !entries.length && (
            <div className="empty-state">
              <h3>{t(subscriptions.length ? "noFilter" : "followEmpty")}</h3>
              <p>
                {t(subscriptions.length ? "followSchedule" : "followEmptyHint")}
              </p>
              {!subscriptions.length && (
                <Button onClick={onSearch}>{t("followAdd")}</Button>
              )}
              {!!subscriptions.length && (selected !== "all" || unread) && (
                <Button
                  onClick={() => {
                    setSelected("all");
                    setUnread(false);
                    setStart(0);
                  }}
                >
                  {t("resetFilters")}
                </Button>
              )}
            </div>
          )}
          {(start > 0 || entries.length === 50) && (
            <div className="query-pagination">
              <Button
                disabled={loading || start === 0}
                onClick={() => setStart((n) => n - 50)}
              >
                {t("previous")}
              </Button>
              <span>{t("page", { page: start / 50 + 1 })}</span>
              <Button
                disabled={loading || entries.length < 50}
                onClick={() => setStart((n) => n + 50)}
              >
                {t("next")}
              </Button>
            </div>
          )}
        </>
      )}
      <Dialog
        open={!!editing}
        onOpenChange={(_, d) => {
          if (!d.open && !following.busy) setEditing(undefined);
        }}
      >
        <DialogSurface>
          <DialogBody>
            <DialogTitle>{t("followEdit")}</DialogTitle>
            <DialogContent>
              <div className="live-form">
                {following.error && (
                  <MessageBar intent="error">
                    {explain(following.error)}
                  </MessageBar>
                )}
                <p>
                  {editing?.account.riotId} · {editing?.account.platform}
                </p>
                <Field label={t("followLabel")}>
                  <Input
                    value={label}
                    maxLength={80}
                    placeholder={t("followLabelHint")}
                    onChange={(_, d) => setLabel(d.value)}
                    disabled={following.busy}
                  />
                </Field>
                <Switch
                  label={t("followPause")}
                  checked={paused}
                  onChange={(_, d) => setPaused(d.checked)}
                  disabled={following.busy}
                />
                <p className="muted">{t("followKeep")}</p>
                <Button
                  disabled={following.busy}
                  onClick={() =>
                    void following
                      .run("unfollow_player", { id: editing!.id })
                      .then((ok) => {
                        if (ok) {
                          setEditing(undefined);
                          setSelected("all");
                          setStart(0);
                        }
                      })
                  }
                >
                  {t("followRemove")}
                </Button>
              </div>
            </DialogContent>
            <DialogActions>
              <Button
                disabled={following.busy}
                onClick={() => setEditing(undefined)}
              >
                {t("cancel")}
              </Button>
              <Button
                appearance="primary"
                disabled={following.busy}
                onClick={() =>
                  void following
                    .run("edit_following", { id: editing!.id, label, paused })
                    .then((ok) => {
                      if (ok) setEditing(undefined);
                    })
                }
              >
                {t("sessionConfirm")}
              </Button>
            </DialogActions>
          </DialogBody>
        </DialogSurface>
      </Dialog>
    </div>
  );
}
