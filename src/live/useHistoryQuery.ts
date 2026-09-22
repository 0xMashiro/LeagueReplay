import {
  useEffect,
  useEffectEvent,
  useLayoutEffect,
  useRef,
  useState,
  type RefObject,
} from "react";
import { invoke, type Call } from "../ipc";
import type { Region } from "../generated/library/Region";
import type { SearchResult } from "../generated/library/SearchResult";
import type { LiveStore } from "./useLiveWorkspace";
import type { PlayerHistoryRequest } from "./PlayerHistoryLink";

export type HistoryViewState = {
  request?: PlayerHistoryRequest;
  server: string;
  riotId: string;
  result?: SearchResult;
  selected: string[];
  scrollTop: number;
  error: string;
};

export function useHistoryQuery(
  store: LiveStore,
  snapshot: RefObject<HistoryViewState>,
) {
  const [regions, setRegions] = useState<Region[]>([]);
  const [server, setServer] = useState(snapshot.current.server);
  const [riotId, setRiotId] = useState(snapshot.current.riotId);
  const [result, setResult] = useState(snapshot.current.result);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState(snapshot.current.error);
  const revision = useRef(0);
  const [selected, setSelected] = useState(snapshot.current.selected);
  const myAccounts = store.workspace?.ui.myAccounts ?? [];
  useLayoutEffect(() => {
    snapshot.current = {
      ...snapshot.current,
      server,
      riotId,
      result,
      selected,
      error,
    };
  }, [snapshot, server, riotId, result, selected, error]);
  useEffect(
    () => () => {
      revision.current++;
    },
    [],
  );
  useEffect(() => {
    let disposed = false;
    void invoke("query_regions")
      .then((items) => {
        if (!disposed) {
          setRegions(items);
          setServer(
            (current) => current || items.find((r) => r.current)?.id || "",
          );
        }
      })
      .catch((reason) => {
        if (!disposed) setError(String(reason));
      });
    return () => {
      disposed = true;
    };
  }, [store.workspace?.status.account?.id]);
  const query = async (
    call: Call<
      "search_player" | "search_current_player" | "player_history_page"
    >,
    refresh = false,
  ) => {
    const current = ++revision.current;
    setLoading(true);
    setError("");
    try {
      const found = await invoke(...call);
      if (current !== revision.current) return;
      // Some cross-region endpoints omit the tag; keep a known full Riot ID.
      const known =
        result?.account.id === found.account.id
          ? result.account
          : myAccounts.find((s) => s.account.id === found.account.id)?.account;
      if (!found.account.summonerId && known?.summonerId)
        found.account.summonerId = known.summonerId;
      if (
        known &&
        (found.account.riotId === found.account.puuid ||
          (!found.account.riotId.includes("#") && known.riotId.includes("#")))
      )
        found.account.riotId = known.riotId;
      setResult(found);
      setSelected((ids) =>
        refresh
          ? ids.filter((id) => found.games.some((g) => g.gameId === id))
          : [],
      );
      if (!refresh) snapshot.current.scrollTop = 0;
      setServer(found.server);
      setRiotId(found.account.riotId);
      if (
        myAccounts.some(
          (s) =>
            s.account.id === found.account.id &&
            JSON.stringify(s.account) !== JSON.stringify(found.account),
        )
      )
        store.update((ui) => ({
          ...ui,
          myAccounts: ui.myAccounts.map((s) =>
            s.account.id === found.account.id
              ? { server: found.server, account: found.account }
              : s,
          ),
        }));
    } catch (reason) {
      if (current === revision.current) setError(String(reason));
    } finally {
      if (current === revision.current) setLoading(false);
    }
  };
  const queryPending = useEffectEvent((request: PlayerHistoryRequest) => {
    void query(
      request.puuid
        ? [
            "player_history_page",
            { server: request.server, puuid: request.puuid, start: 0 },
          ]
        : ["search_player", { server: request.server, riotId: request.riotId }],
    );
  });
  useEffect(() => {
    const request = snapshot.current.request;
    if (!request) return;
    snapshot.current.request = undefined;
    queryPending(request);
  }, [snapshot]);
  return {
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
  };
}
