import { useSyncExternalStore } from "react";
import { invoke } from "../ipc";
import type { ReplayJob } from "../generated/library/ReplayJob";
let snapshot: { jobs: ReplayJob[]; error: string } = { jobs: [], error: "" };
const subscribers = new Set<() => void>();
let generation = 0;
let timer: ReturnType<typeof setTimeout> | undefined;
async function poll(ticket: number) {
  let next: typeof snapshot;
  try {
    next = { jobs: await invoke("replay_jobs"), error: "" };
  } catch (error) {
    next = { ...snapshot, error: String(error) };
  }
  if (ticket !== generation) return;
  snapshot = next;
  for (const notify of subscribers) notify();
  timer = setTimeout(() => void poll(ticket), 1000);
}
function subscribe(notify: () => void) {
  subscribers.add(notify);
  if (subscribers.size === 1) void poll(++generation);
  return () => {
    subscribers.delete(notify);
    if (!subscribers.size) {
      generation++;
      clearTimeout(timer);
    }
  };
}
export function useReplayJobs() {
  return useSyncExternalStore(subscribe, () => snapshot);
}
