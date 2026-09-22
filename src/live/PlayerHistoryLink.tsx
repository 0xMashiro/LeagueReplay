import {
  createContext,
  useContext,
  useEffect,
  useState,
  type ReactNode,
} from "react";
import { isTauri } from "@tauri-apps/api/core";
import { invoke } from "../ipc";
import { Tooltip } from "@fluentui/react-components";
import type { Region } from "../generated/library/Region";
import { useI18n } from "../i18n";

export type PlayerHistoryTarget = {
  platform: string;
  puuid?: string;
  riotId: string;
};
export type PlayerHistoryRequest = PlayerHistoryTarget & { server: string };
const HistoryContext = createContext<{
  regions: Region[];
  ready: boolean;
  open: (target: PlayerHistoryRequest) => void;
} | null>(null);

export function PlayerHistoryProvider({
  children,
  clientKey,
  onOpen,
}: {
  children: ReactNode;
  clientKey: string;
  onOpen: (target: PlayerHistoryRequest) => void;
}) {
  const [availability, setAvailability] = useState<{
    key: string;
    regions: Region[];
    ready: boolean;
  }>({ key: "", regions: [], ready: false });
  useEffect(() => {
    let disposed = false;
    if (isTauri())
      void invoke("query_regions")
        .then((regions) => {
          if (!disposed)
            setAvailability({ key: clientKey, regions, ready: true });
        })
        .catch(() => {
          if (!disposed)
            setAvailability({ key: clientKey, regions: [], ready: true });
        });
    return () => {
      disposed = true;
    };
  }, [clientKey]);
  const current = availability.key === clientKey;
  return (
    <HistoryContext.Provider
      value={{
        regions: current ? availability.regions : [],
        ready: current && availability.ready,
        open: onOpen,
      }}
    >
      {children}
    </HistoryContext.Provider>
  );
}

export function PlayerHistoryLink({
  target,
  children,
}: {
  target: PlayerHistoryTarget;
  children?: ReactNode;
}) {
  const context = useContext(HistoryContext);
  const { t } = useI18n();
  const region = context?.regions.find(
    (r) => r.id === target.platform || r.platform === target.platform,
  );
  const puuid =
    target.puuid &&
    /^[A-Za-z0-9_-]{1,128}$/.test(target.puuid) &&
    target.puuid !== "00000000-0000-0000-0000-000000000000"
      ? target.puuid
      : undefined;
  const identified = !!puuid || /^[^#]+#[^#]+$/.test(target.riotId);
  const available = identified && region?.available;
  const reason = !identified
    ? t("playerIdentityMissing")
    : !context?.ready
      ? t("loading")
      : !region?.available
        ? t("switchClient")
        : t("playerHistory");
  return (
    <Tooltip content={reason} relationship="description">
      <button
        type="button"
        className="player-history-link"
        aria-disabled={!available}
        onClick={() => {
          if (available && region)
            context?.open({ ...target, puuid, server: region.id });
        }}
      >
        {children ?? target.riotId}
      </button>
    </Tooltip>
  );
}
