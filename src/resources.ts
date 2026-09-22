import { useSyncExternalStore } from "react";
import bundled from "./data/catalog.json";
import names from "./data/asset-names.json";
import type { ResourceData } from "./generated/resources/ResourceData";
export type { ResourceData } from "./generated/resources/ResourceData";
let current: ResourceData = {
  catalog: bundled,
  names: { en: names.en, ja: names.ja, ko: names.ko },
};
const listeners = new Set<() => void>();
const subscribe = (notify: () => void) => {
  listeners.add(notify);
  return () => {
    listeners.delete(notify);
  };
};
export const resourceData = () => current;
export const useResources = () => useSyncExternalStore(subscribe, resourceData);
export function applyResources(value: ResourceData | null) {
  if (value) {
    current = value;
    for (const notify of listeners) notify();
  }
}
export function bundledImage(kind: "champion" | "item", id: string | number) {
  const entries: Record<string, { bundled: boolean }> =
    kind === "champion" ? bundled.champions : bundled.items;
  return entries[id]?.bundled ? `/assets/${id}.png` : undefined;
}
