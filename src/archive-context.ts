import { createContext, useContext } from "react";
import type { Account, ArchiveState } from "./domain/types";

interface ArchiveStore {
  state: ArchiveState;
  update: (change: (state: ArchiveState) => ArchiveState) => void;
  saving: boolean;
  error: string;
  retry: () => void;
  accounts: Record<string, Account>;
}

// Keep context identity separate from the hot-reloaded provider component.
export const ArchiveContext = createContext<ArchiveStore | null>(null);

export function useArchive(): ArchiveStore {
  const store = useContext(ArchiveContext);
  if (!store) throw new Error("Review context is required");
  return store;
}
