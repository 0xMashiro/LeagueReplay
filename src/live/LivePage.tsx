import { lazy, Suspense, type RefObject } from "react";
import { isTauri } from "@tauri-apps/api/core";
import { Button, MessageBar, Spinner } from "@fluentui/react-components";
import { ui, useI18n } from "../i18n";
import type { Page } from "../domain/types";
import type { LiveStore } from "./useLiveWorkspace";
import type { HistoryViewState } from "./useHistoryQuery";

const PlayPage = lazy(() =>
  import("./PlayPage").then((m) => ({ default: m.PlayPage })),
);
const SettingsPage = lazy(() =>
  import("./SettingsPage").then((m) => ({ default: m.SettingsPage })),
);
const HistoryPanel = lazy(() =>
  import("./HistoryPanel").then((m) => ({ default: m.HistoryPanel })),
);
const ReviewLibrary = lazy(() =>
  import("./ReviewLibrary").then((m) => ({ default: m.ReviewLibrary })),
);
const FollowingPanel = lazy(() =>
  import("./FollowingPanel").then((m) => ({ default: m.FollowingPanel })),
);

export function LivePage({
  historyView,
  page,
  store,
  onSearch,
  onDirtyChange,
  onNavigate,
}: {
  historyView: RefObject<HistoryViewState>;
  page: Page;
  store: LiveStore;
  onSearch: () => void;
  onDirtyChange: (dirty: boolean) => void;
  onNavigate: (action: () => void) => void;
}) {
  const { error: explain } = useI18n();
  const view = store.workspace;
  if (!isTauri())
    return (
      <div className="empty-state">
        <h3>{ui("在桌面版中连接游戏客户端")}</h3>
        <p>{ui("请打开桌面应用以查询战绩、管理录像和记录游玩。")}</p>
      </div>
    );
  if (!view)
    return (
      <div className="empty-state">
        {store.error ? (
          <MessageBar intent="error">{explain(store.error)}</MessageBar>
        ) : (
          <Spinner label={ui("正在读取本地游玩归档…")} />
        )}
        <Button onClick={() => void store.refresh()}>{ui("重新读取")}</Button>
      </div>
    );
  const props = { store, onSearch, onDirtyChange, onNavigate };
  return (
    <Suspense
      fallback={
        <div className="empty-state">
          <Spinner label={ui("正在读取本地游玩归档…")} />
        </div>
      }
    >
      {page === "settings" ? (
        <SettingsPage view={view} store={store} />
      ) : page === "search" ? (
        <HistoryPanel snapshot={historyView} {...props} />
      ) : page === "library" ? (
        <ReviewLibrary {...props} />
      ) : page === "following" ? (
        <FollowingPanel {...props} />
      ) : (
        <PlayPage view={view} {...props} />
      )}
    </Suspense>
  );
}
