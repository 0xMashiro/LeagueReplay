import { useCallback, useEffect, useRef, useState } from "react";
import { isTauri } from "@tauri-apps/api/core";
import { invoke, type Call, type Result } from "../ipc";
import { IpcError } from "../ipc-error";
import type { LiveWorkspace } from "../generated/live/LiveWorkspace";
import type { LiveUiState } from "../generated/live/LiveUiState";
import { mergeUi } from "./mergeUi";
import { mergeWorkspace } from "./workspace-update";

export function useLiveWorkspace() {
  const [workspace, setWorkspace] = useState<LiveWorkspace>();
  const [error, setError] = useState("");
  const [saving, setSaving] = useState(false);
  const [busy, setBusy] = useState(false);
  const [conflict, setConflict] = useState<{
    base: LiveUiState;
    mine: LiveUiState;
    saved: LiveUiState;
  }>();
  const base = useRef<LiveUiState | undefined>(undefined);
  const latest = useRef<LiveWorkspace | undefined>(undefined);
  const pending = useRef<LiveUiState | undefined>(undefined);
  const writing = useRef(false);
  const generation = useRef(0);
  const mounted = useRef(true);
  const actionError = useRef(false);
  const executing = useRef(false);
  const cursor = useRef<number | null>(null);
  const refresh = useCallback(async () => {
    if (!isTauri() || pending.current || writing.current) return;
    const epoch = generation.current;
    try {
      const delta = await invoke("sync_workspace", { cursor: cursor.current });
      if (mounted.current && epoch === generation.current && !pending.current) {
        const data = mergeWorkspace(latest.current, delta);
        cursor.current = delta.cursor;
        latest.current = data;
        base.current = data.ui;
        setWorkspace(data);
        if (!actionError.current) setError("");
      }
    } catch (reason) {
      if (mounted.current) setError(String(reason));
    }
  }, []);
  useEffect(() => {
    mounted.current = true;
    let timer: ReturnType<typeof setTimeout>;
    let cancelled = false;
    const poll = async () => {
      await refresh();
      if (!cancelled) timer = setTimeout(poll, 4000);
    };
    void poll();
    return () => {
      mounted.current = false;
      cancelled = true;
      clearTimeout(timer);
    };
  }, [refresh]);

  const flush = useCallback(async () => {
    if (writing.current || !latest.current) return;
    writing.current = true;
    setSaving(true);
    try {
      let rebases = 0;
      while (pending.current && latest.current) {
        const ui = pending.current;
        let revision: number;
        try {
          revision = await invoke("save_live_ui", {
            ui,
            revision: latest.current.uiRevision,
          });
        } catch (reason) {
          if (
            !(reason instanceof IpcError && reason.code === "ui.conflict") ||
            !base.current ||
            rebases++ >= 3
          )
            throw reason;
          const remote = await invoke("live_workspace");
          const mine = pending.current!;
          const combined = mergeUi(base.current, mine, remote.ui);
          const conflictData = { base: base.current, mine, saved: remote.ui };
          base.current = remote.ui;
          latest.current = { ...remote, ui: combined.ui };
          pending.current = combined.ui;
          setWorkspace(latest.current);
          if (combined.conflicts.length) {
            setConflict(conflictData);
            setError("");
            return;
          }
          continue;
        }
        base.current = ui;
        latest.current = { ...latest.current, uiRevision: revision };
        if (pending.current === ui) pending.current = undefined;
      }
      setError("");
    } catch (reason) {
      setError(String(reason));
    } finally {
      writing.current = false;
      setSaving(false);
    }
  }, []);
  const update = useCallback(
    (change: (ui: LiveUiState) => LiveUiState) => {
      if (!latest.current || conflict) return;
      generation.current++;
      const ui = change(latest.current.ui);
      pending.current = ui;
      latest.current = { ...latest.current, ui };
      setWorkspace(latest.current);
      void flush();
    },
    [flush, conflict],
  );
  const command = async <const T extends Call>(
    call: T,
    onResult?: (result: Result<T[0]>) => void,
    onFailure?: (reason: string) => void,
  ) => {
    if (executing.current) return false;
    if (pending.current || writing.current) {
      setError("ui.pendingSave");
      onFailure?.("ui.pendingSave");
      return false;
    }
    executing.current = true;
    setBusy(true);
    actionError.current = false;
    setError("");
    generation.current++;
    try {
      const result = await invoke<T>(...call);
      onResult?.(result);
      await refresh();
      return true;
    } catch (reason) {
      actionError.current = true;
      setError(String(reason));
      onFailure?.(String(reason));
      return false;
    } finally {
      executing.current = false;
      setBusy(false);
    }
  };
  useEffect(() => {
    const prevent = (event: BeforeUnloadEvent) => {
      if (pending.current || writing.current) event.preventDefault();
    };
    window.addEventListener("beforeunload", prevent);
    return () => window.removeEventListener("beforeunload", prevent);
  }, []);
  return {
    workspace,
    error,
    saving,
    hasPending: !!pending.current,
    conflict,
    resolveConflict: async (prefer: "mine" | "saved") => {
      if (!conflict || !latest.current) return;
      const combined = mergeUi(
        conflict.base,
        conflict.mine,
        conflict.saved,
        prefer,
      );
      pending.current = combined.ui;
      latest.current = { ...latest.current, ui: combined.ui };
      setWorkspace(latest.current);
      setConflict(undefined);
      await flush();
    },
    busy,
    update,
    command,
    refresh,
    retry: async () => {
      if (conflict) return;
      if (pending.current) await flush();
      else {
        actionError.current = false;
        await refresh();
      }
    },
  };
}
export type LiveStore = ReturnType<typeof useLiveWorkspace>;
