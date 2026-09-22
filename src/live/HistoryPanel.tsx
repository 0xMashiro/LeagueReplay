import { useLayoutEffect, useRef, useState, type RefObject } from "react";
import {
  Badge,
  Button,
  Dropdown,
  Field,
  Input,
  MessageBar,
  Option,
  OptionGroup,
  Spinner,
  Checkbox,
  Tooltip,
} from "@fluentui/react-components";
import {
  Search20Regular,
  Dismiss16Regular,
  ArrowClockwise20Regular,
  PersonAdd20Regular,
} from "@fluentui/react-icons";
import type { Region } from "../generated/library/Region";
import { championAlias } from "./adapter";
import type { LiveStore } from "./useLiveWorkspace";
import { useI18n } from "../i18n";
import { useReview } from "./useReview";
import { ReviewPanel, type ReviewNavigation } from "./ReviewPanel";
import { useFollowing } from "./useFollowing";
import { MatchRow } from "./MatchRow";
import { OrganizeMatchesDialog } from "./OrganizeMatchesDialog";
import { useReplayActions } from "./useReplayActions";
import { queueLabel } from "../game-labels";
import { useHistoryQuery, type HistoryViewState } from "./useHistoryQuery";

export function HistoryPanel({
  store,
  snapshot,
  ...navigation
}: ReviewNavigation & {
  store: LiveStore;
  snapshot: RefObject<HistoryViewState>;
}) {
  const { t, language, error: explain } = useI18n();
  const {
    regions,
    server,
    setServer,
    riotId,
    setRiotId,
    result,
    loading,
    error,
    selected,
    setSelected,
    myAccounts,
    query,
  } = useHistoryQuery(store, snapshot);
  const scroll = useRef<HTMLDivElement>(null);
  const returnToMatch = useRef<string | undefined>(undefined);
  const review = useReview();
  const following = useFollowing();
  const replay = useReplayActions();
  useLayoutEffect(() => {
    if (!scroll.current) return;
    scroll.current.scrollTop = snapshot.current.scrollTop;
    if (returnToMatch.current && result) {
      const index = result.games.findIndex(
        (g) => g.gameId === returnToMatch.current,
      );
      const buttons = scroll.current.querySelectorAll<HTMLButtonElement>(
        ".archive-match-main",
      );
      buttons[index]?.focus({ preventScroll: true });
      returnToMatch.current = undefined;
    }
  }, [snapshot, review.game, result]);
  const importable = selected.filter(
    (id) =>
      !store.workspace?.matches.some(
        (m) =>
          m.id === `${result?.account.platform}_${id}` &&
          m.accountId === result?.account.id,
      ),
  );
  const [backfilling, setBackfilling] = useState(false);
  const region = regions.find((r) => r.id === server);
  const regionName = (value: Region) =>
    language === "zh-CN" ? value.name : value.englishName;
  const regionGroups = [true, false]
    .map((tencent) => ({
      tencent,
      items: regions
        .filter((item) => item.id.startsWith("TENCENT_") === tencent)
        .toSorted(
          (a, b) =>
            Number(b.current) - Number(a.current) ||
            Number(b.available) - Number(a.available),
        ),
    }))
    .filter((group) => group.items.length > 0)
    .toSorted(
      (a, b) =>
        Number(b.items.some((item) => item.current || item.available)) -
        Number(a.items.some((item) => item.current || item.available)),
    );
  const disabled = loading || review.busy || replay.busy || store.busy;
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
    <div
      className="page-scroll live-page search-page"
      ref={scroll}
      onScroll={(event) => {
        snapshot.current.scrollTop = event.currentTarget.scrollTop;
      }}
    >
      <header className="live-heading">
        <div>
          <h1>{t("search")}</h1>
        </div>
        <Button
          disabled={disabled}
          onClick={() => void query(["search_current_player"])}
        >
          {t("current")}
        </Button>
      </header>
      <div className="query-panel">
        <form
          className="query-form"
          onSubmit={(event) => {
            event.preventDefault();
            if (!disabled && region?.available && riotId.trim())
              void query(["search_player", { server, riotId }]);
          }}
        >
          <Field label={t("region")}>
            <Dropdown
              className="query-region"
              listbox={{ className: "region-listbox" }}
              positioning={{
                position: "below",
                align: "start",
                autoSize: "width",
                fallbackPositions: ["above"],
              }}
              aria-label={t("region")}
              disabled={disabled}
              value={region ? regionName(region) : ""}
              selectedOptions={server ? [server] : []}
              onOptionSelect={(_, data) => setServer(data.optionValue!)}
            >
              {regionGroups.map((group) => (
                <OptionGroup
                  key={String(group.tencent)}
                  label={t(group.tencent ? "regionTencent" : "regionRiot")}
                >
                  {group.items.map((r) => (
                    <Option
                      key={r.id}
                      value={r.id}
                      text={regionName(r)}
                      disabled={!r.available}
                    >
                      {regionName(r)}
                      {r.current
                        ? ` · ${t("regionCurrent")}`
                        : !r.available
                          ? ` · ${t("switchClient")}`
                          : ""}
                    </Option>
                  ))}
                </OptionGroup>
              ))}
            </Dropdown>
          </Field>
          <Field label={t("riotId")}>
            <Input
              value={riotId}
              disabled={disabled}
              maxLength={161}
              placeholder={t("enterId")}
              onChange={(_, data) => setRiotId(data.value)}
            />
          </Field>
          <Button
            type="submit"
            appearance="primary"
            icon={<Search20Regular />}
            disabled={disabled || !region?.available || !riotId.trim()}
          >
            {t("find")}
          </Button>
        </form>
        <p className="muted query-hint">{t("regionHint")}</p>
      </div>
      <section className="my-accounts" aria-label={t("myAccounts")}>
        <div className="my-accounts-heading">
          <h2>{t("myAccounts")}</h2>
          {!myAccounts.length && (
            <span className="muted">{t("myAccountsEmpty")}</span>
          )}
        </div>
        {myAccounts.length > 0 && (
          <div className="my-account-list">
            {myAccounts.map((saved) => {
              const savedRegion = regions.find((r) => r.id === saved.server);
              const name = `${saved.account.riotId} · ${savedRegion ? regionName(savedRegion) : saved.account.platform}`;
              return (
                <div className="my-account" key={saved.account.id}>
                  <Tooltip
                    content={
                      savedRegion?.available ? t("find") : t("switchClient")
                    }
                    relationship="description"
                  >
                    <Button
                      appearance="subtle"
                      disabled={disabled || !savedRegion?.available}
                      onClick={() =>
                        void query([
                          "player_history_page",
                          {
                            server: saved.server,
                            puuid: saved.account.puuid,
                            start: 0,
                          },
                        ])
                      }
                    >
                      {name}
                    </Button>
                  </Tooltip>
                  <Tooltip
                    content={t("removeMyAccount", {
                      name: saved.account.riotId,
                    })}
                    relationship="label"
                  >
                    <Button
                      appearance="subtle"
                      size="small"
                      icon={<Dismiss16Regular />}
                      aria-label={t("removeMyAccount", {
                        name: saved.account.riotId,
                      })}
                      disabled={store.busy}
                      onClick={() =>
                        store.update((ui) => ({
                          ...ui,
                          myAccounts: ui.myAccounts.filter(
                            (s) => s.account.id !== saved.account.id,
                          ),
                        }))
                      }
                    />
                  </Tooltip>
                </div>
              );
            })}
          </div>
        )}
      </section>

      {(error || review.error || following.error) && (
        <MessageBar intent="error">
          {explain(error || review.error || following.error)}
          {error &&
            result &&
            ` ${t("searchKept", { name: result.account.riotId })}`}
        </MessageBar>
      )}
      {(loading || review.busy) && (
        <Spinner size="small" label={t("loading")} />
      )}
      {result && (
        <>
          <div className="query-result-heading">
            <h2>{result.account.riotId}</h2>
            <Badge appearance="outline" color="informative">
              {regions.find((r) => r.id === result.server)
                ? regionName(regions.find((r) => r.id === result.server)!)
                : result.account.platform}
            </Badge>
            <div className="query-account-actions">
              {myAccounts.some((s) => s.account.id === result.account.id) ? (
                <Badge appearance="outline">{t("myAccountAdded")}</Badge>
              ) : (
                <Tooltip
                  content={t("myAccountsHint")}
                  relationship="description"
                >
                  <Button
                    icon={<PersonAdd20Regular />}
                    disabled={store.busy}
                    onClick={() =>
                      store.update((ui) => ({
                        ...ui,
                        myAccounts: [
                          ...ui.myAccounts.filter(
                            (s) => s.account.id !== result.account.id,
                          ),
                          { server: result.server, account: result.account },
                        ],
                      }))
                    }
                  >
                    {t("addMyAccount")}
                  </Button>
                </Tooltip>
              )}
              {following.state.subscriptions.some(
                (s) => s.account.id === result.account.id,
              ) ? (
                <Badge appearance="outline">{t("followAdded")}</Badge>
              ) : (
                <Button
                  disabled={following.busy}
                  onClick={() =>
                    void following.run("follow_player", {
                      server: result.server,
                      puuid: result.account.puuid,
                    })
                  }
                >
                  {t("followAdd")}
                </Button>
              )}
              <Tooltip content={t("refreshHistory")} relationship="label">
                <Button
                  icon={<ArrowClockwise20Regular />}
                  aria-label={t("refreshHistory")}
                  disabled={disabled}
                  onClick={() =>
                    void query(
                      [
                        "player_history_page",
                        {
                          server: result.server,
                          puuid: result.account.puuid,
                          start: result.start,
                        },
                      ],
                      true,
                    )
                  }
                />
              </Tooltip>
            </div>
          </div>
          {result.skippedGames > 0 && (
            <MessageBar intent="warning">
              {t("historyPartial", { count: result.skippedGames })}
            </MessageBar>
          )}
          <div className="selection-toolbar">
            <Checkbox
              label={t("selectPage")}
              checked={
                selected.length > 0 && selected.length === result.games.length
              }
              disabled={disabled}
              onChange={(_, d) =>
                setSelected(d.checked ? result.games.map((g) => g.gameId) : [])
              }
            />
            <span>
              {selected.length > 0
                ? t("selectedMatches", { count: selected.length })
                : ""}
            </span>
            {selected.length > 0 && (
              <>
                <Button
                  disabled={disabled}
                  onClick={() =>
                    void replay.download(
                      selected.map((id) => ({
                        id: result.account.platform + "_" + id,
                        source: {
                          server: result.server,
                          gameId: id,
                          puuid: result.account.puuid,
                        },
                      })),
                    )
                  }
                >
                  {t("batchDownload")}
                </Button>
                <Button
                  disabled={disabled || !importable.length}
                  onClick={() => setBackfilling(true)}
                >
                  {t("batchBackfill")}
                </Button>
              </>
            )}
          </div>
          {replay.error && (
            <MessageBar intent="error">{explain(replay.error)}</MessageBar>
          )}
          {backfilling && (
            <OrganizeMatchesDialog
              store={store}
              ids={importable}
              backfill={result}
              onClose={(completed) => {
                setBackfilling(false);
                if (completed) setSelected([]);
              }}
            />
          )}
          <div className="query-results" aria-busy={disabled}>
            {result.games.map(({ gameId, game }) => {
              const me = game.participants.find(
                (p) => p.puuid === result.account.puuid,
              );
              const hero = championAlias(me?.championId ?? 0);
              const recorded = store.workspace?.matches.find(
                (m) =>
                  m.id === `${result.account.platform}_${gameId}` &&
                  m.accountId === result.account.id,
              );
              const id = gameId,
                qualified = result.account.platform + "_" + id;
              return (
                <MatchRow
                  key={id}
                  matchId={qualified}
                  riotId={result.account.riotId}
                  onReveal={() => void replay.reveal(qualified)}
                  summary={{
                    champion: hero,
                    startedAt: game.startedAt,
                    duration: game.duration,
                    queue: queueLabel(game.queueId, ""),
                    win: me?.win ?? undefined,
                    kills: me?.kills ?? 0,
                    deaths: me?.deaths ?? 0,
                    assists: me?.assists ?? 0,
                    items: me?.items ?? [],
                  }}
                  provenance={
                    recorded
                      ? t(recorded.manual ? "imported" : "recorded")
                      : undefined
                  }
                  note={
                    store.workspace?.ui.notes.find(
                      (n) => n.matchId === qualified,
                    )?.body
                  }
                  selected={selected.includes(id)}
                  onSelect={(checked) =>
                    setSelected((ids) =>
                      checked
                        ? [...new Set([...ids, id])]
                        : ids.filter((value) => value !== id),
                    )
                  }
                  job={replay.jobs.find((j) => j.id === qualified)}
                  disabled={disabled}
                  onOpen={
                    !me
                      ? undefined
                      : () => {
                          returnToMatch.current = id;
                          void review.open("open_review_game", {
                            server: result.server,
                            gameId: id,
                            puuid: result.account.puuid,
                            refresh: false,
                          });
                        }
                  }
                  onReplay={
                    !me
                      ? undefined
                      : () =>
                          void replay.run(qualified, {
                            server: result.server,
                            gameId: id,
                            puuid: result.account.puuid,
                          })
                  }
                />
              );
            })}
          </div>
          {!result.games.length && !loading && (
            <div className="empty-state">
              <h3>{t("noMatches")}</h3>
              <p>{t("noMatchesHint")}</p>
            </div>
          )}
          <div className="query-pagination">
            <Button
              disabled={disabled || result.start === 0}
              onClick={() =>
                void query([
                  "player_history_page",
                  {
                    server: result.server,
                    puuid: result.account.puuid,
                    start: result.start - 20,
                  },
                ])
              }
            >
              {t("previous")}
            </Button>
            <span>{t("page", { page: result.start / 20 + 1 })}</span>
            <Button
              disabled={disabled || !result.hasMore}
              onClick={() =>
                void query([
                  "player_history_page",
                  {
                    server: result.server,
                    puuid: result.account.puuid,
                    start: result.start + 20,
                  },
                ])
              }
            >
              {t("next")}
            </Button>
          </div>
        </>
      )}
      {!result && !loading && (
        <div className="empty-state">
          <Search20Regular />
          <h3>{t("emptySearch")}</h3>
          <p>{t("emptyHint")}</p>
        </div>
      )}
    </div>
  );
}
