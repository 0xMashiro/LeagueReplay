import { useSyncExternalStore } from "react";

export type ThemePreference = "dark" | "light" | "system";
const key = "league-replay:theme";
const valid = (value: unknown): value is ThemePreference =>
  value === "dark" || value === "light" || value === "system";
function read(): ThemePreference | undefined {
  try {
    const value = localStorage.getItem(key);
    return valid(value) ? value : undefined;
  } catch {
    return undefined;
  }
}
let preference = read() ?? "dark";
const listeners = new Set<() => void>();
export function setThemePreference(value: ThemePreference) {
  preference = value;
  try {
    localStorage.setItem(key, value);
  } catch {
    /* Keep the selected theme for this session. */
  }
  listeners.forEach((listener) => listener());
}
export function useThemePreference() {
  return useSyncExternalStore(
    (listener) => {
      listeners.add(listener);
      return () => {
        listeners.delete(listener);
      };
    },
    () => preference,
  );
}
