import { useEffect, useRef, useState, type ReactNode } from "react";
import {
  Badge,
  Button,
  Checkbox,
  Tooltip,
  Menu,
  MenuTrigger,
  MenuPopover,
  MenuList,
  MenuItem,
  MenuDivider,
  Toast,
  ToastTitle,
  useToastController,
} from "@fluentui/react-components";
import {
  ArrowDownload20Regular,
  Play20Regular,
  Note20Regular,
  MoreHorizontal20Regular,
  Copy20Regular,
  FolderOpen20Regular,
} from "@fluentui/react-icons";
import { Champion, championName, ItemBuild } from "../components/Assets";
import { formatTime } from "../domain/archive";
import { useI18n } from "../i18n";
import type { ReplayJob } from "../generated/library/ReplayJob";
export type MatchSummary = {
  champion: string;
  startedAt: number | string;
  queue: string;
  duration?: number;
  win?: boolean;
  kills?: number;
  deaths?: number;
  assists?: number;
  items?: number[];
};
export function MatchRow({
  summary: s,
  identity,
  note,
  provenance,
  job,
  onOpen,
  onReplay,
  menuItems,
  matchId,
  rowKey,
  riotId,
  onReveal,
  selected,
  onSelect,
  disabled = false,
  timeOnly = false,
}: {
  summary: MatchSummary;
  identity?: string;
  note?: string;
  provenance?: string;
  job?: ReplayJob;
  onOpen?: () => void;
  onReplay?: () => void;
  menuItems?: ReactNode;
  matchId?: string;
  rowKey?: string;
  riotId?: string;
  onReveal?: () => void;
  selected?: boolean;
  onSelect?: (selected: boolean) => void;
  disabled?: boolean;
  timeOnly?: boolean;
}) {
  const { t, date, language } = useI18n();
  const { dispatchToast } = useToastController("feedback");
  const [menuOpen, setMenuOpen] = useState(false);
  const [point, setPoint] = useState<{ x: number; y: number }>();
  const trigger = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    if (disabled) setMenuOpen(false);
  }, [disabled]);
  const copy = async (text: string) => {
    try {
      await navigator.clipboard.writeText(text);
      dispatchToast(
        <Toast>
          <ToastTitle>{t("copied")}</ToastTitle>
        </Toast>,
        { intent: "success" },
      );
    } catch {
      dispatchToast(
        <Toast>
          <ToastTitle>{t("copyFailed")}</ToastTitle>
        </Toast>,
        { intent: "error" },
      );
    }
  };
  const active =
    !!job &&
    ["waiting", "queued", "downloading", "validating", "cancelling"].includes(
      job.state,
    );
  const progressLabels: Record<string, string> = {
    waiting: t("waitingClient"),
    queued: t("queued"),
    validating: t("validating"),
    cancelling: t("cancelling"),
  };
  const progressLabel = progressLabels[job?.state ?? ""] ?? t("downloading");
  const replayLabel =
    job?.state === "ready"
      ? t("playReplay")
      : active
        ? progressLabel
        : t("download");
  return (
    <article
      className={`archive-match ${selected ? "is-selected" : ""}`}
      data-result={s.win === undefined ? "pending" : s.win ? "win" : "loss"}
      data-match-key={rowKey ?? matchId}
      data-menu-open={menuOpen || undefined}
      onContextMenu={(event) => {
        // Portal events bubble through React; only handle the actual row surface.
        if (
          disabled ||
          !event.currentTarget.contains(event.target as Node) ||
          (event.target as HTMLElement).closest(
            "input, textarea, select, [contenteditable=true]",
          )
        )
          return;
        event.preventDefault();
        trigger.current?.focus({ preventScroll: true });
        setPoint({ x: event.clientX, y: event.clientY });
        setMenuOpen(true);
      }}
      onKeyDown={(event) => {
        if (
          disabled ||
          !event.currentTarget.contains(event.target as Node) ||
          (event.target as HTMLElement).closest(
            "input, textarea, select, [contenteditable=true]",
          )
        )
          return;
        if (
          event.key === "ContextMenu" ||
          (event.shiftKey && event.key === "F10")
        ) {
          event.preventDefault();
          setPoint(undefined);
          trigger.current?.focus({ preventScroll: true });
          setMenuOpen(true);
        }
      }}
    >
      {onSelect && (
        <Checkbox
          aria-label={t("selectMatch", {
            name: championName(s.champion),
            time: date(s.startedAt),
          })}
          checked={selected ?? false}
          disabled={disabled}
          onChange={(_, d) => onSelect(d.checked === true)}
        />
      )}
      <button
        className="archive-match-main"
        disabled={!onOpen || disabled}
        onClick={onOpen}
        aria-label={t("openMatch", {
          name: championName(s.champion),
          time: date(s.startedAt),
        })}
      >
        <span
          className={`archive-match-result ${s.win === undefined ? "muted" : s.win ? "win-text" : "loss-text"}`}
        >
          <strong>
            {s.win === undefined ? t("pending") : t(s.win ? "win" : "loss")}
          </strong>
          <small>
            {timeOnly
              ? new Date(s.startedAt).toLocaleTimeString(language, {
                  hour: "2-digit",
                  minute: "2-digit",
                })
              : s.duration
                ? formatTime(s.duration)
                : ""}
          </small>
        </span>
        <Champion id={s.champion} size={44} />
        <span className="archive-match-name">
          <strong>{identity || championName(s.champion)}</strong>
          <small>
            {identity ? championName(s.champion) + " · " : ""}
            {s.queue} ·{" "}
            {timeOnly
              ? s.duration
                ? formatTime(s.duration)
                : "—"
              : date(s.startedAt)}
          </small>
        </span>
        <span className="archive-match-kda numeric">
          <strong>
            {s.kills ?? "—"} / {s.deaths ?? "—"} / {s.assists ?? "—"}
          </strong>
          {s.kills !== undefined && (
            <small>
              {(
                (s.kills + (s.assists ?? 0)) /
                Math.max(1, s.deaths ?? 0)
              ).toFixed(2)}{" "}
              KDA
            </small>
          )}
        </span>
      </button>
      <div className="archive-match-items">
        {s.items && <ItemBuild items={s.items} />}
      </div>
      <div className="archive-match-status">
        {provenance && <small>{provenance}</small>}
        {job && (
          <Badge
            appearance="outline"
            color={job.state === "ready" ? "informative" : "subtle"}
          >
            {active
              ? progressLabel
              : t(
                  job.state === "ready"
                    ? "downloaded"
                    : job.state === "missing"
                      ? "missing"
                      : job.state === "error"
                        ? "retry"
                        : job.state === "cancelled"
                          ? "cancelled"
                          : "downloading",
                )}
          </Badge>
        )}
      </div>
      <div className="archive-match-actions">
        {onReplay && (
          <Tooltip content={replayLabel} relationship="label">
            <Button
              aria-label={replayLabel}
              appearance="subtle"
              icon={
                job?.state === "ready" ? (
                  <Play20Regular />
                ) : (
                  <ArrowDownload20Regular />
                )
              }
              disabled={disabled || active}
              onClick={onReplay}
            />
          </Tooltip>
        )}
        <Menu
          open={menuOpen}
          closeOnScroll
          onOpenChange={(_, data) => {
            setMenuOpen(data.open);
            if (!data.open) setPoint(undefined);
          }}
          positioning={{
            position: "below",
            align: "end",
            target: point
              ? {
                  getBoundingClientRect: () =>
                    DOMRect.fromRect({
                      x: point.x,
                      y: point.y,
                      width: 0,
                      height: 0,
                    }),
                }
              : undefined,
          }}
        >
          <MenuTrigger disableButtonEnhancement>
            <Button
              ref={trigger}
              appearance="subtle"
              icon={<MoreHorizontal20Regular />}
              aria-label={t("moreMatchActions")}
              disabled={disabled}
              onClick={() => setPoint(undefined)}
            />
          </MenuTrigger>
          <MenuPopover className="match-menu">
            <MenuList>
              <MenuItem disabled={!onOpen} onClick={onOpen}>
                {t("review")}
              </MenuItem>
              {onReplay && (
                <MenuItem
                  disabled={active}
                  onClick={onReplay}
                  icon={
                    job?.state === "ready" ? (
                      <Play20Regular />
                    ) : (
                      <ArrowDownload20Regular />
                    )
                  }
                >
                  {replayLabel}
                </MenuItem>
              )}
              {onReveal && job?.state === "ready" && (
                <MenuItem onClick={onReveal} icon={<FolderOpen20Regular />}>
                  {t("reveal")}
                </MenuItem>
              )}
              {menuItems && (
                <>
                  <MenuDivider />
                  {menuItems}
                </>
              )}
              {(riotId || matchId) && <MenuDivider />}
              {riotId && (
                <MenuItem
                  icon={<Copy20Regular />}
                  onClick={() => void copy(riotId)}
                >
                  {t("copyRiotId")}
                </MenuItem>
              )}
              {matchId && (
                <MenuItem
                  icon={<Copy20Regular />}
                  onClick={() => void copy(matchId)}
                >
                  {t("copyMatchId")}
                </MenuItem>
              )}
            </MenuList>
          </MenuPopover>
        </Menu>
      </div>
      {note && (
        <button
          className="archive-note-preview"
          onClick={onOpen}
          disabled={!onOpen || disabled}
        >
          <Note20Regular aria-hidden="true" />
          <span className="archive-note-label">{t("notes")}</span>
          <span className="archive-note-body">{note}</span>
          {onOpen && (
            <span className="archive-note-action">{t("review")} →</span>
          )}
        </button>
      )}
    </article>
  );
}
