import { invoke } from "../ipc";
import { useRef, useState } from "react";
import {
  Toast,
  ToastTitle,
  useToastController,
} from "@fluentui/react-components";
import { useI18n } from "../i18n";
import { useReplayJobs } from "./useReplayJobs";
export function useReplayActions() {
  const { t } = useI18n();
  const { dispatchToast } = useToastController("feedback");
  const locked = useRef(false);
  const { jobs, error: pollingError } = useReplayJobs();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const prepare = async (
    id: string,
    source?: { server: string; gameId: string; puuid: string },
  ) => {
    if (source) await invoke("open_review_game", { ...source, refresh: false });
    else await invoke("saved_review_game", { id });
  };
  const run = async (
    id: string,
    source?: { server: string; gameId: string; puuid: string },
  ) => {
    if (locked.current) return;
    locked.current = true;
    setBusy(true);
    setError("");
    try {
      const ready = jobs.some((job) => job.id === id && job.state === "ready");
      await prepare(id, ready ? undefined : source);
      await invoke(ready ? "open_replay" : "download_replay", { id });
      if (!ready)
        dispatchToast(
          <Toast>
            <ToastTitle>{t("replayQueued")}</ToastTitle>
          </Toast>,
          { intent: "success" },
        );
    } catch (reason) {
      setError(String(reason));
    } finally {
      locked.current = false;
      setBusy(false);
    }
  };
  const download = async (
    items: {
      id: string;
      source?: { server: string; gameId: string; puuid: string };
    }[],
  ) => {
    if (locked.current) return;
    locked.current = true;
    setBusy(true);
    setError("");
    try {
      for (const item of items) {
        if (
          jobs.some(
            (job) =>
              job.id === item.id &&
              [
                "ready",
                "waiting",
                "queued",
                "downloading",
                "validating",
                "cancelling",
              ].includes(job.state),
          )
        )
          continue;
        await prepare(item.id, item.source);
        await invoke("download_replay", { id: item.id });
      }
    } catch (reason) {
      setError(String(reason));
    } finally {
      locked.current = false;
      setBusy(false);
    }
  };
  const reveal = async (id: string) => {
    if (locked.current) return;
    locked.current = true;
    setBusy(true);
    setError("");
    try {
      await invoke("reveal_replay", { id });
    } catch (reason) {
      setError(String(reason));
    } finally {
      locked.current = false;
      setBusy(false);
    }
  };
  const bookmark = async (id: string, bookmarked: boolean) => {
    if (locked.current) return false;
    locked.current = true;
    setBusy(true);
    setError("");
    try {
      await invoke("bookmark_review_game", { id, bookmarked });
      return true;
    } catch (reason) {
      setError(String(reason));
      return false;
    } finally {
      locked.current = false;
      setBusy(false);
    }
  };
  return {
    jobs,
    busy,
    error: error || pollingError,
    run,
    download,
    reveal,
    bookmark,
  };
}
