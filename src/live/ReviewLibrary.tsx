import { useEffect, useState } from "react";
import { invoke } from "../ipc";
import {
  Button,
  MessageBar,
  MenuItem,
  Spinner,
  Tab,
  TabList,
} from "@fluentui/react-components";
import type { LibraryEntry } from "../generated/library/LibraryEntry";
import { championAlias } from "./adapter";
import { useI18n } from "../i18n";
import { exportJson } from "../export-file";
import { ReviewPanel, type ReviewNavigation } from "./ReviewPanel";
import { useReview } from "./useReview";
import { useReplayJobs } from "./useReplayJobs";
import type { LiveStore } from "./useLiveWorkspace";
import { ReplayTasks } from "./ReplayTasks";
import { MatchRow } from "./MatchRow";
import { queueLabel } from "../game-labels";
import { useReplayActions } from "./useReplayActions";
import { SearchField } from "../components/SearchField";
import { useListReturn } from "./useListReturn";

export function ReviewLibrary({
  store,
  onSearch,
  ...navigation
}: ReviewNavigation & { store: LiveStore; onSearch: () => void }) {
  const { t, date, error: explain } = useI18n();
  const review = useReview();
  const list = useListReturn(!!review.game);
  const replay = useReplayActions();
  const { jobs, error: jobError } = useReplayJobs();
  const [filter, setFilter] = useState("recent");
  const [start, setStart] = useState(0);
  const [entries, setEntries] = useState<LibraryEntry[]>([]);
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(false);
  const [attempt, setAttempt] = useState(0);
  const [query, setQuery] = useState("");
  const [search, setSearch] = useState("");
  useEffect(() => {
    setEntries([]);
  }, [filter, start, search]);
  useEffect(() => {
    const timer = setTimeout(() => {
      setStart(0);
      setSearch(query.trim());
    }, 250);
    return () => clearTimeout(timer);
  }, [query]);
  const activeJobs = jobs.filter((job) =>
    ["waiting", "queued", "downloading", "validating"].includes(job.state),
  ).length;
  const completed = jobs
    .filter((job) => job.state === "ready")
    .map((job) => job.id)
    .sort()
    .join("|");
  useEffect(() => {
    if (review.game || filter === "notes" || filter === "tasks") return;
    let disposed = false;
    setLoading(true);
    setError("");
    void invoke("list_library", {
      start,
      filter,
      query: search,
    })
      .then((rows) => {
        if (!disposed) setEntries(rows);
      })
      .catch((reason) => {
        if (!disposed) setError(String(reason));
      })
      .finally(() => {
        if (!disposed) setLoading(false);
      });
    return () => {
      disposed = true;
    };
  }, [filter, start, attempt, review.game, completed, search]);
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
  const notes = store
    .workspace!.ui.notes.filter((n) =>
      `${n.body} ${n.tags.join(" ")}`
        .toLocaleLowerCase()
        .includes(query.toLocaleLowerCase()),
    )
    .toSorted((a, b) => b.updatedAt.localeCompare(a.updatedAt));
  return (
    <div className="page-scroll live-page library-page" ref={list.scrollRef}>
      <header className="live-heading">
        <div>
          <h1>{t("library")}</h1>
        </div>
        <Button
          disabled={!store.workspace!.ui.notes.length}
          onClick={() =>
            exportJson(store.workspace!.ui.notes, "LeagueReplay-notes.json")
          }
        >
          {t("exportNotes")}
        </Button>
      </header>
      <div className="view-controls">
        <div className="page-toolbar">
          <TabList
            selectedValue={filter}
            onTabSelect={(_, data) => {
              setFilter(String(data.value));
              setStart(0);
              setEntries([]);
            }}
          >
            <Tab value="recent">{t("recent")}</Tab>
            <Tab value="saved">{t("saved")}</Tab>
            <Tab value="downloaded">{t("downloaded")}</Tab>
            <Tab value="tasks">
              {t("downloads")}
              {activeJobs > 0 && (
                <span
                  className="tab-count"
                  aria-label={t("taskActiveCount", { count: activeJobs })}
                >
                  {activeJobs}
                </span>
              )}
            </Tab>
            <Tab value="notes">{t("notes")}</Tab>
          </TabList>
          {filter !== "notes" && filter !== "tasks" && (
            <Button
              appearance="subtle"
              onClick={() => setAttempt((n) => n + 1)}
              disabled={loading}
            >
              {t("refresh")}
            </Button>
          )}
        </div>
        {!["notes", "tasks"].includes(filter) && (
          <SearchField
            className="notes-search"
            label={t("librarySearch")}
            value={query}
            onChange={setQuery}
          />
        )}
      </div>
      {(error || review.error || jobError || replay.error) && (
        <MessageBar intent="error">
          {explain(error || review.error || jobError || replay.error)}{" "}
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
      {(loading || review.busy) && (
        <Spinner size="small" label={t("loading")} />
      )}
      {filter === "tasks" ? (
        <ReplayTasks
          jobs={jobs}
          onReview={(id) => {
            list.remember(id);
            void review.open("saved_review_game", { id });
          }}
        />
      ) : filter === "notes" ? (
        <>
          <SearchField
            className="notes-search"
            label={t("searchNotes")}
            value={query}
            onChange={setQuery}
          />
          {notes.map((note) => (
            <article className="review-note" key={note.id}>
              <div>
                <small className="muted">
                  {note.tags.join(" · ") || t("notes")} · {date(note.updatedAt)}
                </small>
                <p>{note.body}</p>
                <small className="muted">
                  {note.matchId}
                  {note.at === undefined
                    ? ""
                    : ` · ${Math.floor(note.at / 60)}:${String(note.at % 60).padStart(2, "0")}`}
                </small>
              </div>
              <Button
                data-return-key={note.id}
                disabled={review.busy}
                onClick={() => {
                  list.remember(note.id);
                  void review.open("saved_review_game", { id: note.matchId });
                }}
              >
                {t("review")}
              </Button>
            </article>
          ))}
          {!notes.length && (
            <div className="empty-state">
              <h3>{t("noMatchingNotes")}</h3>
              {query && (
                <Button onClick={() => setQuery("")}>{t("clearSearch")}</Button>
              )}
            </div>
          )}
        </>
      ) : (
        <>
          <div className="query-results" aria-busy={loading}>
            {entries.map((entry) => {
              const hero = championAlias(entry.championId),
                job = jobs.find((j) => j.id === entry.id);
              return (
                <MatchRow
                  key={entry.id}
                  matchId={entry.id}
                  riotId={entry.account.riotId}
                  summary={{
                    ...entry,
                    champion: hero,
                    queue: queueLabel(entry.queueId, ""),
                    items: entry.items,
                  }}
                  identity={entry.account.riotId}
                  provenance={entry.bookmarked ? t("saved") : undefined}
                  note={
                    store.workspace?.ui.notes.find(
                      (n) => n.matchId === entry.id,
                    )?.body
                  }
                  job={job}
                  disabled={review.busy || loading || replay.busy}
                  onOpen={() => {
                    list.remember(entry.id);
                    void review.open("saved_review_game", {
                      id: entry.id,
                      puuid: entry.account.puuid,
                    });
                  }}
                  onReplay={() => void replay.run(entry.id)}
                  onReveal={() => void replay.reveal(entry.id)}
                  menuItems={
                    <MenuItem
                      onClick={() =>
                        void replay
                          .bookmark(entry.id, !entry.bookmarked)
                          .then((saved) => {
                            if (saved) setAttempt((n) => n + 1);
                          })
                      }
                    >
                      {t(entry.bookmarked ? "unsave" : "save")}
                    </MenuItem>
                  }
                />
              );
            })}
          </div>
          {!loading && !error && !entries.length && (
            <div className="empty-state">
              <h3>
                {t(filter === "recent" && !query ? "noLibrary" : "noFilter")}
              </h3>
              {query || filter !== "recent" ? (
                <Button
                  onClick={() => {
                    setQuery("");
                    setSearch("");
                    setFilter("recent");
                    setStart(0);
                  }}
                >
                  {t("resetFilters")}
                </Button>
              ) : (
                <>
                  <p>{t("noLibraryHint")}</p>
                  <Button onClick={onSearch}>{t("search")}</Button>
                </>
              )}
            </div>
          )}
          {(start > 0 || entries.length >= 50) && (
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
    </div>
  );
}
