import { useEffect, useRef, useState } from "react";
import { invoke, type Call } from "../ipc";
import type { ReviewGame } from "../generated/library/ReviewGame";

export function useReview() {
  const [game, setGame] = useState<ReviewGame>();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const generation = useRef(0);
  useEffect(
    () => () => {
      generation.current++;
    },
    [],
  );
  const open = async (
    ...call: Call<"open_review_game" | "saved_review_game">
  ) => {
    const current = ++generation.current;
    setBusy(true);
    setError("");
    try {
      const result = await invoke(...call);
      if (current === generation.current) setGame(result);
    } catch (reason) {
      if (current === generation.current) setError(String(reason));
    } finally {
      if (current === generation.current) setBusy(false);
    }
  };
  return { game, setGame, busy, error, open };
}
