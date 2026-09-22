import { ui, useI18n } from "../i18n";
import { useMemo, useState } from "react";
import {
  Button,
  Dropdown,
  Option,
  Slider,
  Tooltip,
} from "@fluentui/react-components";
import { ArrowReset20Regular, ZoomIn20Regular } from "@fluentui/react-icons";
import { groupPurchases, formatTime } from "../domain/archive";
import type { Match, MatchEvent } from "../domain/types";
import { Champion, championName, Item, itemInfo } from "./Assets";

export function ItemTimeline({
  match,
  onSelect,
}: {
  match: Match;
  onSelect: (event: MatchEvent, label: string) => void;
}) {
  const { t } = useI18n();
  const [zoom, setZoom] = useState(1);
  const [filter, setFilter] = useState("all");
  const [selected, setSelected] = useState<MatchEvent[]>();
  const groups = useMemo(() => groupPurchases(match.events), [match.events]);
  const ticks = Array.from(
    { length: Math.floor(match.duration / 300) + 1 },
    (_, i) => i * 300,
  );
  const select = (group: MatchEvent[]) => {
    const primary = group.toSorted(
      (a, b) => itemInfo(b.itemId!).gold - itemInfo(a.itemId!).gold,
    )[0];
    setSelected(group);
    onSelect(
      primary,
      `${championName(match.participants.find((person) => person.id === primary.participantId)!.champion)} · ${itemInfo(primary.itemId!).name}`,
    );
  };
  if (!match.timelineAvailable)
    return (
      <div className="empty-state">
        <ZoomIn20Regular />
        <h3>{ui("这局没有收录过程数据")}</h3>
        <p>{ui("战绩和整局笔记仍然可用，出装顺序需要对局时间线。")}</p>
      </div>
    );
  return (
    <section className="timeline-section">
      <div className="timeline-toolbar">
        <div>
          <h2>{t("itemChanges")}</h2>
          <p>{t("itemChangesHint")}</p>
        </div>
        <Dropdown
          className="timeline-filter"
          aria-label={ui("时间线玩家筛选")}
          value={
            filter === "all"
              ? ui("全部玩家")
              : filter === "mine"
                ? ui("仅关注玩家")
                : ui("同位置对比")
          }
          selectedOptions={[filter]}
          onOptionSelect={(_, data) => setFilter(data.optionValue!)}
        >
          <Option value="all">{ui("全部玩家")}</Option>
          <Option value="mine">{ui("仅关注玩家")}</Option>
          <Option value="role">{ui("同位置对比")}</Option>
        </Dropdown>
      </div>
      <div className="timeline-scroll">
        <div className="timeline-canvas" style={{ width: `${zoom * 100}%` }}>
          <div className="timeline-axis">
            <div className="timeline-label">{ui("玩家 / 游戏时间")}</div>
            <div className="timeline-track">
              {ticks.map((tick) => (
                <span
                  key={tick}
                  style={{ left: `${(tick / match.duration) * 100}%` }}
                >
                  {formatTime(tick)}
                </span>
              ))}
            </div>
          </div>
          {match.participants.map((participant) => {
            const muted =
              filter === "mine"
                ? participant.id !== match.participantId
                : filter === "role"
                  ? participant.role !==
                    match.participants.find(
                      (person) => person.id === match.participantId,
                    )!.role
                  : false;
            return (
              <div
                className={`timeline-lane ${participant.id === match.participantId ? "is-perspective" : ""} ${muted ? "is-muted" : ""}`}
                key={participant.id}
              >
                <div className="timeline-label">
                  <Champion id={participant.champion} size={28} />
                  <span>
                    {championName(participant.champion)}
                    <small>
                      {participant.role}
                      {participant.id === match.participantId
                        ? ui(" · 关注玩家")
                        : ""}
                    </small>
                  </span>
                  <i className={`team-marker team-${participant.team}`} />
                </div>
                <div className="timeline-track">
                  {ticks.map((tick) => (
                    <span
                      className="timeline-gridline"
                      key={tick}
                      style={{ left: `${(tick / match.duration) * 100}%` }}
                    />
                  ))}
                  {groups
                    .filter(
                      (group) => group[0].participantId === participant.id,
                    )
                    .map((group) => {
                      const primary = group.toSorted(
                        (a, b) =>
                          itemInfo(b.itemId!).gold - itemInfo(a.itemId!).gold,
                      )[0];
                      const label = `${championName(participant.champion)} ${formatTime(primary.at)} ${primary.label} ${group.map((event) => itemInfo(event.itemId!).name).join("、")}`;
                      return (
                        <Tooltip
                          content={label}
                          relationship="label"
                          key={primary.id}
                        >
                          <button
                            className={`purchase item-event-${primary.kind} ${itemInfo(primary.itemId!).finished ? "finished-item" : ""} ${selected?.some((event) => event.id === primary.id) ? "selected-purchase" : ""}`}
                            style={{
                              left: `${Math.max(2, Math.min(97, (primary.at / match.duration) * 100))}%`,
                            }}
                            aria-label={label}
                            aria-pressed={
                              selected?.some(
                                (event) => event.id === primary.id,
                              ) ?? false
                            }
                            onClick={() => select(group)}
                          >
                            <Item id={primary.itemId!} size={24} />
                            {primary.kind !== "purchase" && (
                              <em className="item-change-mark">
                                {primary.kind === "sale"
                                  ? "−"
                                  : primary.kind === "undo"
                                    ? "↶"
                                    : "×"}
                              </em>
                            )}
                            {group.length > 1 && (
                              <span>+{group.length - 1}</span>
                            )}
                          </button>
                        </Tooltip>
                      );
                    })}
                </div>
              </div>
            );
          })}
        </div>
      </div>
      <div className="timeline-footer">
        <div className="timeline-legend">
          <span className="legend-square" />
          {ui("成装")}
          <span className="legend-square component" />
          {ui("组件 / 消耗品")}
          <span className="muted">{t("itemGroupHint")}</span>
        </div>
        <div className="timeline-zoom">
          <ZoomIn20Regular />
          <Slider
            aria-label={ui("时间线缩放")}
            min={1}
            max={3}
            step={0.5}
            value={zoom}
            onChange={(_, data) => setZoom(data.value)}
          />
          <span>{zoom}×</span>
          <Tooltip content={ui("适配整局")} relationship="label">
            <Button
              size="small"
              appearance="subtle"
              icon={<ArrowReset20Regular />}
              aria-label={ui("适配整局")}
              onClick={() => setZoom(1)}
            />
          </Tooltip>
        </div>
      </div>
      {selected && (
        <div className="selected-purchase-detail">
          <span className="time-pill">{formatTime(selected[0].at)}</span>
          <span>
            {
              match.participants.find(
                (person) => person.id === selected[0].participantId,
              )?.name
            }
          </span>
          {selected.map((event) => (
            <span key={event.id}>
              <Item id={event.itemId!} size={24} />
              {event.label} · {itemInfo(event.itemId!).name}
              {!!event.restoredItemId &&
                event.restoredItemId !== event.itemId && (
                  <>
                    {" "}
                    → <Item id={event.restoredItemId} size={24} />
                    {itemInfo(event.restoredItemId).name}
                  </>
                )}
            </span>
          ))}
        </div>
      )}
    </section>
  );
}
