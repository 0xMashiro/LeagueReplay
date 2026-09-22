import { useLayoutEffect, useRef } from "react";

/** Keep the row and scroll position when returning from match details. */
export function useListReturn(detailOpen: boolean) {
  const scrollRef = useRef<HTMLDivElement>(null);
  const saved = useRef<{ top: number; key?: string } | null>(null);
  const remember = (key?: string) => {
    saved.current = { top: scrollRef.current?.scrollTop ?? 0, key };
  };
  useLayoutEffect(() => {
    if (detailOpen || !saved.current || !scrollRef.current) return;
    const { top, key } = saved.current;
    if (key)
      scrollRef.current
        .querySelector<HTMLElement>(
          `[data-match-key="${CSS.escape(key)}"] .archive-match-main, [data-return-key="${CSS.escape(key)}"]`,
        )
        ?.focus({ preventScroll: true });
    scrollRef.current.scrollTop = top;
    saved.current = null;
  }, [detailOpen]);
  return { scrollRef, remember };
}
