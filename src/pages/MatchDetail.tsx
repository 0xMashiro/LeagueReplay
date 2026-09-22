import { ui, getLanguage, useI18n } from "../i18n";
import { inlineNotesQuery } from "../layout";
import { PlayerHistoryLink } from "../live/PlayerHistoryLink";
import { useEffect, useRef, useState, type ReactNode } from "react";
import {
  Badge,
  Button,
  Tab,
  TabList,
  Drawer,
  DrawerHeader,
  DrawerHeaderTitle,
  DrawerBody,
} from "@fluentui/react-components";
import {
  ArrowDownload20Regular,
  ArrowLeft20Regular,
  Checkmark20Regular,
  Note20Regular,
  Trophy20Regular,
  Dismiss20Regular,
} from "@fluentui/react-icons";
import { formatTime } from "../domain/archive";
import type { Match, MatchEvent } from "../domain/types";
import { useArchive } from "../archive-context";
import { Champion, championName, ItemBuild } from "../components/Assets";
import { ItemTimeline } from "../components/ItemTimeline";
import {
  NotesPanel,
  type NoteContext,
  type NoteDraft,
} from "../components/NotesPanel";

export function MatchDetail({
  match,
  onBack,
  onDownload,
  onDirtyChange,
  actions,
  onSeek,
}: {
  match: Match;
  actions?: ReactNode;
  onSeek?: (at: number) => void;
  onBack: () => void;
  onDownload: (id: string) => void;
  onDirtyChange: (dirty: boolean) => void;
}) {
  const { t } = useI18n();
  const notesDraft = useRef<NoteDraft | undefined>(undefined);
  const [wide, setWide] = useState(
    () => window.matchMedia(inlineNotesQuery).matches,
  );
  useEffect(() => {
    const mq = window.matchMedia(inlineNotesQuery);
    const change = () => setWide(mq.matches);
    mq.addEventListener("change", change);
    return () => mq.removeEventListener("change", change);
  }, []);
  const { state, update, accounts } = useArchive();
  const [tab, setTab] = useState("overview");
  const [context, setContext] = useState<NoteContext>();
  const [notesOpen, setNotesOpen] = useState(wide);
  const backButton = useRef<HTMLButtonElement>(null);
  const notesButton = useRef<HTMLButtonElement>(null);
  const notesWasOpen = useRef(false);
  useEffect(() => {
    if (!notesOpen && notesWasOpen.current)
      notesButton.current?.focus({ preventScroll: true });
    notesWasOpen.current = notesOpen;
  }, [notesOpen]);
  useEffect(() => {
    backButton.current?.focus({ preventScroll: true });
  }, []);
  const account = accounts[match.accountId];
  const me = match.participants.find(
    (person) => person.id === match.participantId,
  )!;
  const reviewed = state.reviewed.includes(match.id);
  useEffect(() => () => onDirtyChange(false), [onDirtyChange]);
  const select = (event: MatchEvent, label: string) => {
    setContext({
      at: event.at,
      participantId: event.participantId,
      label,
      nonce: Date.now(),
    });
    setNotesOpen(true);
  };
  const playerKills = match.events.filter(
    (event) =>
      event.kind === "multikill" && event.participantId === match.participantId,
  );
  return (
    <div className="detail-page">
      <div className="detail-breadcrumb">
        <Button
          ref={backButton}
          appearance="subtle"
          icon={<ArrowLeft20Regular />}
          onClick={onBack}
        >
          {ui("返回列表")}
        </Button>
        <span>
          <PlayerHistoryLink
            target={{
              platform: account.platform,
              riotId: account.riotId,
              puuid: me.puuid,
            }}
          />
          <b>/</b>
          {new Date(match.startedAt).toLocaleString(getLanguage(), {
            month: "2-digit",
            day: "2-digit",
            hour: "2-digit",
            minute: "2-digit",
          })}
        </span>
        <Badge appearance="outline" color="informative" size="small">
          {ui("客户端对局")}
        </Badge>
      </div>
      <header
        className="match-heading"
        data-result={match.win ? "win" : "loss"}
      >
        <div
          className={`detail-result ${match.win ? "win-text" : "loss-text"}`}
        >
          {match.win ? ui("胜利") : ui("失败")}
          <small>{match.queue}</small>
        </div>
        <Champion id={match.champion} size={64} />
        <div className="detail-champion">
          <h1>{championName(match.champion)}</h1>
          <p>
            {account.region} · {formatTime(match.duration)} · {match.patch}
          </p>
        </div>
        <div className="detail-kda">
          <strong>
            {me.kills}
            <i> / {me.deaths} / </i>
            {me.assists}
          </strong>
          <span>
            {((me.kills + me.assists) / Math.max(1, me.deaths)).toFixed(2)} KDA
          </span>
        </div>
        <div className="detail-achievements">
          {match.multikills &&
            (
              [
                ["double", "双杀"],
                ["triple", "三杀"],
                ["quadra", "四杀"],
                ["penta", "五杀"],
              ] as const
            )
              .filter(([key]) => match.multikills![key] > 0)
              .map(([key, label]) => (
                <Badge
                  key={key}
                  icon={<Trophy20Regular />}
                  appearance="tint"
                  color="brand"
                >
                  {ui(label)} ×{match.multikills![key]}
                </Badge>
              ))}
          {playerKills.map((event) => (
            <Badge
              key={event.id}
              icon={<Trophy20Regular />}
              appearance="tint"
              color="brand"
            >
              {event.label.split(" · ")[0]}
            </Badge>
          ))}
        </div>
        <div className="detail-primary">
          {actions !== undefined ? (
            actions
          ) : (
            <>
              <Button
                appearance={reviewed ? "secondary" : "subtle"}
                icon={<Checkmark20Regular />}
                onClick={() =>
                  update((current) => ({
                    ...current,
                    reviewed: reviewed
                      ? current.reviewed.filter((id) => id !== match.id)
                      : [...current.reviewed, match.id],
                  }))
                }
              >
                {reviewed ? ui("已复盘") : ui("标记已复盘")}
              </Button>
              <Button
                appearance="primary"
                icon={<ArrowDownload20Regular />}
                disabled={match.replay !== "available"}
                onClick={() => onDownload(match.id)}
              >
                {match.replay === "unavailable"
                  ? ui("录像下载待接入")
                  : match.replay === "expired"
                    ? ui("录像已过期")
                    : ui("下载录像")}
              </Button>
            </>
          )}
        </div>
      </header>
      <div className="detail-body">
        <div className="detail-main">
          <div className="detail-tabs">
            <TabList
              selectedValue={tab}
              onTabSelect={(_, data) => setTab(String(data.value))}
            >
              <Tab value="overview">{ui("对局概览")}</Tab>
              <Tab value="items">{ui("出装时间线")}</Tab>
              <Tab value="events">{ui("关键事件")}</Tab>
            </TabList>
            <Button
              ref={notesButton}
              className="notes-toggle"
              appearance="subtle"
              icon={<Note20Regular />}
              onClick={() => setNotesOpen(!notesOpen)}
            >
              {notesOpen ? ui("收起笔记") : ui("复盘笔记")}
            </Button>
          </div>
          <div className="detail-tab-content">
            {tab === "overview" && (
              <>
                <div className="scoreboard-heading">
                  <h2>
                    {t("scoreboardPlayers", {
                      count: match.participants.length,
                    })}
                  </h2>
                  <span>{ui("伤害 / 补刀 / 最终装备")}</span>
                </div>
                <div className="scoreboard-scroll">
                  <table className="scoreboard">
                    <thead>
                      <tr>
                        <th>{ui("玩家")}</th>
                        <th>K / D / A</th>
                        <th>{ui("伤害")}</th>
                        <th>{ui("补刀")}</th>
                        <th>{ui("装备")}</th>
                      </tr>
                    </thead>
                    <tbody>
                      {match.participants.map((person, index) => (
                        <tr
                          key={person.id}
                          className={`${person.id === match.participantId ? "current-player" : ""} ${index > 0 && person.team !== match.participants[index - 1].team ? "team-break" : ""}`}
                        >
                          <td>
                            <div className="score-player">
                              <span
                                className={`team-marker team-${person.team}`}
                              />
                              <Champion id={person.champion} size={32} />
                              <span>
                                <strong>
                                  <PlayerHistoryLink
                                    target={{
                                      platform: account.platform,
                                      riotId: person.name,
                                      puuid: person.puuid,
                                    }}
                                  />
                                </strong>
                                <small>
                                  {person.team === 100
                                    ? t("blueTeam")
                                    : person.team === 200
                                      ? t("redTeam")
                                      : t("teamNumber", { team: person.team })}
                                  {person.placement
                                    ? ` · ${t("placement", { rank: person.placement })}`
                                    : ""}{" "}
                                  · {person.role} ·{" "}
                                  {championName(person.champion)}
                                </small>
                              </span>
                            </div>
                          </td>
                          <td className="numeric">
                            {person.kills}{" "}
                            <span className="muted">/ {person.deaths} /</span>{" "}
                            {person.assists}
                          </td>
                          <td>
                            <div className={`damage-stat team-${person.team}`}>
                              <span>{(person.damage / 1000).toFixed(1)}k</span>
                              <i
                                style={{
                                  width: `${(person.damage / Math.max(1, ...match.participants.map((player) => player.damage))) * 100}%`,
                                }}
                              />
                            </div>
                          </td>
                          <td className="numeric">{person.cs}</td>
                          <td>
                            <ItemBuild items={person.items} />
                          </td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
                <div className="detail-next">
                  <div>
                    <strong>{ui("看看优势是从哪里建立的")}</strong>
                    <span>{ui("按游戏时间对照双方的关键成装。")}</span>
                  </div>
                  <Button onClick={() => setTab("items")}>
                    {ui("查看出装时间线 →")}
                  </Button>
                </div>
              </>
            )}
            {tab === "items" && (
              <ItemTimeline match={match} onSelect={select} />
            )}
            {tab === "events" && (
              <>
                <div className="scoreboard-heading">
                  <h2>{ui("值得停下来看的时刻")}</h2>
                  <span>{ui("点击事件，添加时间点笔记")}</span>
                </div>
                {match.timelineAvailable ? (
                  <div className="events-list">
                    {match.events
                      .filter((event) =>
                        ["kill", "multikill", "objective"].includes(event.kind),
                      )
                      .toSorted((a, b) => a.at - b.at)
                      .map((event) => {
                        const person = match.participants.find(
                          (player) => player.id === event.participantId,
                        )!;
                        return (
                          <button
                            key={event.id}
                            className="event-row"
                            onClick={() => select(event, event.label)}
                          >
                            <span className="time-pill">
                              {formatTime(event.at)}
                            </span>
                            <Champion id={person.champion} size={36} />
                            <span>
                              <strong>{event.label}</strong>
                              <small>
                                {person.name} · {championName(person.champion)}
                              </small>
                            </span>
                            <Note20Regular />
                          </button>
                        );
                      })}
                  </div>
                ) : (
                  <div className="empty-state">
                    <h3>{ui("这局没有收录过程数据")}</h3>
                    <p>{ui("可以继续添加整局笔记。")}</p>
                  </div>
                )}
              </>
            )}
          </div>
        </div>
        <Drawer
          className="review-notes-drawer"
          type={wide ? "inline" : "overlay"}
          open={notesOpen}
          onOpenChange={(_, data) => setNotesOpen(data.open)}
          position="end"
          size="small"
          unmountOnClose={false}
        >
          <DrawerHeader>
            <DrawerHeaderTitle
              action={
                <Button
                  appearance="subtle"
                  aria-label={ui("关闭笔记")}
                  icon={<Dismiss20Regular />}
                  onClick={() => setNotesOpen(false)}
                />
              }
            >
              {ui("复盘笔记")}
            </DrawerHeaderTitle>
          </DrawerHeader>
          <DrawerBody className="notes-drawer-body">
            <NotesPanel
              match={match}
              context={context}
              draft={notesDraft}
              onSeek={onSeek}
              onDirtyChange={onDirtyChange}
            />
          </DrawerBody>
        </Drawer>
      </div>
    </div>
  );
}
