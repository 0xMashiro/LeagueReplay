import { useResources } from "../resources";
import { indexPlay, filterPlay, initialPlayFilters } from "./play-query";
import type { LiveWorkspace } from "../generated/live/LiveWorkspace";
import { ui } from "../i18n";
import { gameLabel } from "../game-labels";
import { useReview } from "./useReview";
import { ReviewPanel } from "./ReviewPanel";
import { useI18n } from "../i18n";
import { useEffect, useMemo, useState } from "react";
import {
  Button,
  Dropdown,
  MessageBar,
  Option,
  Spinner,
  Tab,
  TabList,
  Checkbox,
  MenuItem,
} from "@fluentui/react-components";
import {
  Search20Regular,
  PlayCircle24Regular,
  ArrowSwap20Regular,
} from "@fluentui/react-icons";
import type { ObservedMatch } from "../generated/live/ObservedMatch";
import { Champion } from "../components/Assets";
import { championAlias } from "./adapter";
import { PlayerPanel } from "./PlayerPanel";
import type { LiveStore } from "./useLiveWorkspace";
import { SessionTools } from "./SessionTools";
import { PlayFilters } from "./PlayFilters";
import { MatchRow } from "./MatchRow";
import { OrganizeMatchesDialog } from "./OrganizeMatchesDialog";
import { useReplayActions } from "./useReplayActions";
import { useListReturn } from "./useListReturn";
import { PlayerHistoryLink } from "./PlayerHistoryLink";
import { AssociatePlayer } from "./AssociatePlayer";

export function PlayPage({
  view,
  store,
  onSearch,
  onDirtyChange,
  onNavigate,
}: {
  view: LiveWorkspace;
  store: LiveStore;
  onSearch: () => void;
  onDirtyChange: (dirty: boolean) => void;
  onNavigate: (action: () => void) => void;
}) {
  const review = useReview();
  const listReturn = useListReturn(!!review.game);
  const replay = useReplayActions();
  const [sessionScope, setSessionScope] = useState("all");
  const [selected, setSelected] = useState<string[]>([]);
  const [moving, setMoving] = useState<string[]>();
  const { t, language, error: explain } = useI18n();
  const [marking, setMarking] = useState<ObservedMatch>();
  const [player, setPlayer] = useState("all");
  const [onlyPremade, setOnlyPremade] = useState(false);
  const [filters, setFilters] = useState(initialPlayFilters);
  const [sessionPage, setSessionPage] = useState(0);
  useEffect(() => {
    if (
      view &&
      player !== "all" &&
      !view.ui.players.some((p) => p.id === player)
    ) {
      setPlayer("all");
      setOnlyPremade(false);
      setSessionPage(0);
    }
    if (
      view &&
      filters.account !== "all" &&
      !view.accounts.some((a) => a.id === filters.account)
    ) {
      setFilters((current) => ({ ...current, account: "all" }));
      setSessionPage(0);
    }
  }, [view, player, filters.account]);
  const resources = useResources();
  const { accounts, matches, sessions: allSessions, ui: annotations } = view;
  const index = useMemo(
    () =>
      indexPlay(
        { accounts, matches, sessions: allSessions, ui: annotations },
        language,
        resources,
      ),
    [accounts, matches, allSessions, annotations, language, resources],
  );
  const profile = view.ui.players.find((p) => p.id === player);
  const { recordsBySegment, sessions } = useMemo(
    () =>
      filterPlay(
        index,
        filters,
        profile?.id ?? "all",
        onlyPremade,
        sessionScope,
      ),
    [index, filters, profile?.id, onlyPremade, sessionScope],
  );
  const {
    accountById,
    accountIds,
    projectedMatches,
    archiveIds,
    settled,
    wins,
    noteCount,
  } = index;
  if (review.game)
    return (
      <ReviewPanel
        game={review.game}
        onChange={review.setGame}
        onBack={() => review.setGame(undefined)}
        store={store}
        onDirtyChange={onDirtyChange}
        onNavigate={onNavigate}
      />
    );
  const currentPage = Math.min(
    sessionPage,
    Math.max(0, Math.ceil(sessions.length / 20) - 1),
  );
  return (
    <div className="page-scroll live-page play-page" ref={listReturn.scrollRef}>
      <header className="live-heading">
        <div>
          <h1>{ui("我的游玩")}</h1>
        </div>
        <Button icon={<Search20Regular />} onClick={onSearch}>
          {ui("查询补录")}
        </Button>
      </header>
      <section className="play-summary" aria-label={t("archiveStory")}>
        <div className="summary-intro">
          <PlayCircle24Regular />
          <span>
            <strong>{t("archiveStory")}</strong>
            <small>{t("archiveStoryHint")}</small>
          </span>
        </div>
        <dl className="archive-overview">
          <div>
            <dt>{t("archiveSessions")}</dt>
            <dd>{view.sessions.length}</dd>
          </div>
          <div>
            <dt>{t("archiveGames")}</dt>
            <dd>{archiveIds.size}</dd>
            <small>
              {t("archiveAccountCount", {
                count: accountIds.size,
              })}
            </small>
          </div>
          <div>
            <dt>{t("archiveWinRate")}</dt>
            <dd className="win-text">
              {settled.length
                ? Math.round((wins / settled.length) * 100) + "%"
                : "—"}
            </dd>
            <small>
              {t("archiveResults", { wins, losses: settled.length - wins })}
            </small>
          </div>
          <div>
            <dt>{t("notes")}</dt>
            <dd>{noteCount}</dd>
          </div>
        </dl>
      </section>
      {review.error && (
        <MessageBar intent="error">{explain(review.error)}</MessageBar>
      )}
      {review.busy && <Spinner size="small" label={t("loading")} />}
      <div className="play-controls">
        <div className="page-toolbar">
          <TabList
            selectedValue={sessionScope}
            onTabSelect={(_, data) => {
              setSessionScope(String(data.value));
              setSessionPage(0);
              setSelected([]);
            }}
          >
            <Tab value="all">{t("allSessions")}</Tab>
            <Tab value="current">{t("currentSession")}</Tab>
            <Tab value="history">{t("historySessions")}</Tab>
          </TabList>
        </div>
        <PlayFilters
          playerFilters={
            <>
              {" "}
              <Dropdown
                aria-label={ui("筛选同队玩家")}
                value={profile?.name ?? ui("全部玩家")}
                selectedOptions={[player]}
                onOptionSelect={(_, data) => {
                  setPlayer(data.optionValue!);
                  setOnlyPremade(false);
                  setSessionPage(0);
                }}
              >
                <Option value="all">{ui("全部玩家")}</Option>
                {view.ui.players.map((p) => (
                  <Option key={p.id} value={p.id}>
                    {p.name}
                  </Option>
                ))}
              </Dropdown>
              {profile && (
                <Checkbox
                  checked={onlyPremade}
                  label={ui("已确认组队")}
                  onChange={(_, d) => setOnlyPremade(d.checked === true)}
                />
              )}
            </>
          }
          playerFilterActive={player !== "all"}
          onResetPlayer={() => {
            setPlayer("all");
            setOnlyPremade(false);
          }}
          value={filters}
          onChange={(value) => {
            setFilters(value);
            setSessionPage(0);
          }}
          accounts={view.accounts.filter((a) => accountIds.has(a.id))}
        />
      </div>
      {replay.error && (
        <MessageBar intent="error">{explain(replay.error)}</MessageBar>
      )}
      {selected.length > 0 && (
        <div className="selection-toolbar">
          <span>{t("selectedMatches", { count: selected.length })}</span>
          <Button onClick={() => setMoving(selected)}>
            {t("moveMatches")}
          </Button>
          <Button
            disabled={replay.busy}
            onClick={() =>
              void replay.download(
                view.matches
                  .filter((m) => selected.includes(m.observationId))
                  .map((m) => ({ id: m.id })),
              )
            }
          >
            {t("batchDownload")}
          </Button>
          <Button appearance="subtle" onClick={() => setSelected([])}>
            {t("clearSelection")}
          </Button>
        </div>
      )}
      {sessions
        .slice(currentPage * 20, currentPage * 20 + 20)
        .map((session) => (
          <section
            className={`session ${session.endedAt === null ? "is-active" : ""}`}
            key={session.id}
          >
            <div className="session-header">
              <div className="session-date">
                <strong>
                  {new Date(session.startedAt).toLocaleDateString(language, {
                    month: "2-digit",
                    day: "2-digit",
                  })}
                </strong>
                <span>
                  {t(
                    session.endedAt === null
                      ? "sessionActive"
                      : "sessionArchived",
                  )}
                </span>
              </div>
              <div className="session-title">
                <h2>{session.title}</h2>
                <span>
                  {new Date(session.startedAt).toLocaleTimeString(language, {
                    hour: "2-digit",
                    minute: "2-digit",
                  })}
                  {session.endedAt !== null && (
                    <>
                      {" "}
                      —{" "}
                      {new Date(session.endedAt).toLocaleTimeString(language, {
                        hour: "2-digit",
                        minute: "2-digit",
                      })}
                    </>
                  )}
                  {" · "}
                  {t("sessionGames", {
                    count: session.segments.reduce(
                      (n, s) => n + s.matchIds.length,
                      0,
                    ),
                  })}
                </span>
              </div>
              <div
                className="session-results"
                role="group"
                aria-label={t("sessionRecentResults")}
              >
                {session.segments
                  .flatMap((segment) =>
                    (recordsBySegment.get(segment.id) ?? []).map((record) =>
                      projectedMatches.get(record.observationId),
                    ),
                  )
                  .filter((match) => match !== undefined)
                  .sort(
                    (a, b) => Date.parse(a.startedAt) - Date.parse(b.startedAt),
                  )
                  .slice(-10)
                  .map((match, index) => (
                    <span
                      key={match.id + index}
                      role="img"
                      aria-label={t(match.win ? "win" : "loss")}
                      title={t(match.win ? "win" : "loss")}
                      className={match.win ? "result-win" : "result-loss"}
                    >
                      {match.win ? "W" : "L"}
                    </span>
                  ))}
              </div>
              {session.endedAt === null && (
                <Button
                  disabled={store.busy}
                  onClick={() =>
                    void store.command([
                      "end_live_session",
                      {
                        sessionId: session.id,
                      },
                    ])
                  }
                >
                  {ui("结束场次")}
                </Button>
              )}
              <SessionTools session={session} store={store} />
            </div>
            <div className="session-content">
              {session.segments.map((segment, segmentIndex) => {
                const records = recordsBySegment.get(segment.id) ?? [];
                if (!records.length) return null;
                return (
                  <div className="account-segment" key={segment.id}>
                    <div className="segment-header">
                      <span className="segment-node" aria-hidden="true">
                        {segmentIndex > 0 ? <ArrowSwap20Regular /> : <span />}
                      </span>
                      <Champion
                        id={championAlias(records[0].championId)}
                        size={24}
                      />
                      <strong>
                        <PlayerHistoryLink
                          target={accountById[segment.accountId]}
                        />
                      </strong>
                      <span className="muted">
                        {accountById[segment.accountId].platform}
                      </span>
                      <AssociatePlayer
                        account={accountById[segment.accountId]}
                        store={store}
                      />
                      <span className="segment-meta">
                        {segmentIndex > 0 && <>{t("accountSwitch")} · </>}
                        {t("sessionGames", { count: records.length })}
                      </span>
                    </div>
                    <div className="segment-matches">
                      {records.map((record) => {
                        const projected = projectedMatches.get(
                          record.observationId,
                        );
                        const me = projected?.participants.find(
                          (p) => p.id === projected.participantId,
                        );
                        return (
                          <div key={record.observationId}>
                            <MatchRow
                              timeOnly
                              summary={{
                                champion:
                                  projected?.champion ??
                                  championAlias(record.championId),
                                startedAt:
                                  projected?.startedAt ?? record.firstSeen,
                                queue:
                                  projected?.queue ??
                                  gameLabel(record.queueName),
                                duration: projected?.duration,
                                win: projected?.win,
                                kills: me?.kills,
                                deaths: me?.deaths,
                                assists: me?.assists,
                                items: me?.items,
                              }}
                              provenance={
                                record.manual ? t("manual") : undefined
                              }
                              note={
                                view.ui.notes.find(
                                  (n) => n.matchId === record.id,
                                )?.body
                              }
                              job={replay.jobs.find((j) => j.id === record.id)}
                              matchId={record.id}
                              rowKey={record.observationId}
                              riotId={accountById[record.accountId]?.riotId}
                              onReveal={() => void replay.reveal(record.id)}
                              selected={selected.includes(record.observationId)}
                              onSelect={
                                record.state === "ready"
                                  ? (checked) =>
                                      setSelected((ids) =>
                                        checked
                                          ? [
                                              ...new Set([
                                                ...ids,
                                                record.observationId,
                                              ]),
                                            ]
                                          : ids.filter(
                                              (id) =>
                                                id !== record.observationId,
                                            ),
                                      )
                                  : undefined
                              }
                              onOpen={
                                record.game
                                  ? () => {
                                      listReturn.remember(record.observationId);
                                      void review.open("saved_review_game", {
                                        id: record.id,
                                        puuid:
                                          accountById[record.accountId].puuid,
                                      });
                                    }
                                  : undefined
                              }
                              onReplay={
                                record.game
                                  ? () => void replay.run(record.id)
                                  : undefined
                              }
                              disabled={
                                review.busy || store.busy || replay.busy
                              }
                              menuItems={
                                <>
                                  <MenuItem
                                    disabled={!record.game}
                                    onClick={() => setMarking(record)}
                                  >
                                    {ui("标记玩家")}
                                  </MenuItem>
                                  <MenuItem
                                    disabled={record.state !== "ready"}
                                    onClick={() =>
                                      setMoving([record.observationId])
                                    }
                                  >
                                    {t("moveMatches")}
                                  </MenuItem>
                                  {record.manual && (
                                    <MenuItem
                                      onClick={() =>
                                        void store.command([
                                          "retract_manual_game",
                                          {
                                            observationId: record.observationId,
                                          },
                                        ])
                                      }
                                    >
                                      {ui("撤销补录")}
                                    </MenuItem>
                                  )}
                                </>
                              }
                            />
                            {record.dataError && (
                              <p className="live-data-error">
                                {explain(record.dataError)}
                              </p>
                            )}
                          </div>
                        );
                      })}
                    </div>
                  </div>
                );
              })}
            </div>
          </section>
        ))}
      {sessions.length > 20 && (
        <div className="query-pagination">
          <Button
            disabled={currentPage === 0}
            onClick={() => setSessionPage(currentPage - 1)}
          >
            {t("previous")}
          </Button>
          <span>{t("page", { page: currentPage + 1 })}</span>
          <Button
            disabled={(currentPage + 1) * 20 >= sessions.length}
            onClick={() => setSessionPage(currentPage + 1)}
          >
            {t("next")}
          </Button>
        </div>
      )}
      {!sessions.length && (
        <div className="empty-state">
          <h3>
            {view.matches.length
              ? ui("没有符合条件的对局")
              : ui("从下一场游玩开始记录")}
          </h3>
          <p>
            {profile
              ? ui("同队来自对局数据，组队需要你明确标记。")
              : view.matches.length
                ? ui("尝试调整关键词、日期或其他筛选条件。")
                : ui(
                    "保持软件运行，进入对局后会按账号自动归档。漏记的对局也可以补录。",
                  )}
          </p>
          {!!view.matches.length && (
            <Button
              onClick={() => {
                setFilters(initialPlayFilters);
                setPlayer("all");
                setOnlyPremade(false);
                setSessionPage(0);
              }}
            >
              {t("resetFilters")}
            </Button>
          )}
          {!view.matches.length && (
            <Button onClick={onSearch}>{ui("查询近期战绩")}</Button>
          )}
        </div>
      )}
      {moving && (
        <OrganizeMatchesDialog
          store={store}
          ids={moving}
          onClose={() => {
            setMoving(undefined);
            setSelected([]);
          }}
        />
      )}
      {marking && (
        <PlayerPanel
          store={store}
          record={marking}
          onClose={() => setMarking(undefined)}
        />
      )}
    </div>
  );
}
