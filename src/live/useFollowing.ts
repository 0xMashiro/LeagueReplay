import { useCallback, useEffect, useRef, useState } from "react";
import { invoke, type Call } from "../ipc";
import type { FollowingState } from "../generated/library/FollowingState";

export function useFollowing() {
  const [state, setState] = useState<FollowingState>({
    subscriptions: [],
    syncing: false,
  });
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const active = useRef(true);
  const running = useRef(false);
  const refresh = useCallback(async () => {
    try {
      const value = await invoke("following_state");
      if (active.current) setState(value);
    } catch (e) {
      if (active.current) setError(String(e));
    }
  }, []);
  useEffect(() => {
    active.current = true;
    let timer: ReturnType<typeof setTimeout>;
    const poll = async () => {
      await refresh();
      if (active.current) timer = setTimeout(poll, 4000);
    };
    void poll();
    return () => {
      active.current = false;
      clearTimeout(timer);
    };
  }, [refresh]);
  const run = async (
    ...call: Call<
      | "follow_player"
      | "edit_following"
      | "unfollow_player"
      | "sync_following"
      | "fill_following_history"
    >
  ) => {
    if (running.current) return false;
    running.current = true;
    setBusy(true);
    setError("");
    try {
      await invoke(...call);
      await refresh();
      return true;
    } catch (e) {
      if (active.current) setError(String(e));
      return false;
    } finally {
      running.current = false;
      if (active.current) setBusy(false);
    }
  };
  return { state, error, busy, run, refresh };
}
