import { ui } from "./i18n";
import { compactSidebarQuery } from "./layout";
import { useEffect, useRef, useState, type ReactElement } from "react";
import type { HistoryViewState } from "./live/useHistoryQuery";
import {
  Button,
  Dialog,
  DialogActions,
  DialogBody,
  DialogContent,
  DialogSurface,
  DialogTitle,
  MessageBar,
  MessageBarActions,
  MessageBarBody,
  Tooltip,
  Toaster,
} from "@fluentui/react-components";
import {
  CalendarLtr24Regular,
  ChevronRight20Regular,
  CircleSmall20Filled,
  Folder24Regular,
  People24Regular,
  Search24Regular,
  Settings24Regular,
} from "@fluentui/react-icons";
import type { Page } from "./domain/types";
import { isTauri } from "@tauri-apps/api/core";
import { invoke } from "./ipc";
import { useLiveWorkspace } from "./live/useLiveWorkspace";
import { LivePage } from "./live/LivePage";
import { useI18n } from "./i18n";
import { DesktopLifecycle } from "./live/DesktopLifecycle";
import { ConflictDialog } from "./live/ConflictDialog";
import { PlayerHistoryProvider } from "./live/PlayerHistoryLink";

import { applyResources } from "./resources";

const navigation: { id: Page; label: string; icon: ReactElement }[] = [
  { id: "play", label: "我的游玩", icon: <CalendarLtr24Regular /> },
  { id: "following", label: "关注动态", icon: <People24Regular /> },
  { id: "search", label: "战绩查询", icon: <Search24Regular /> },
  { id: "library", label: "录像与笔记", icon: <Folder24Regular /> },
];

export function App() {
  useEffect(() => {
    if (isTauri())
      void invoke("cached_resources")
        .then(applyResources)
        .catch(() => {});
  }, []);
  const { error: explainError } = useI18n();
  const live = useLiveWorkspace();
  const [page, setPage] = useState<Page>("play");
  const [historyNavigation, setHistoryNavigation] = useState(0);
  const [compactSidebar, setCompactSidebar] = useState(
    () => matchMedia(compactSidebarQuery).matches,
  );
  const [navTooltip, setNavTooltip] = useState<Page | null>(null);
  useEffect(() => {
    const query = matchMedia(compactSidebarQuery);
    const change = () => {
      setCompactSidebar(query.matches);
      setNavTooltip(null);
    };
    query.addEventListener("change", change);
    return () => query.removeEventListener("change", change);
  }, []);
  const historyView = useRef<HistoryViewState>({
    server: "",
    riotId: "",
    selected: [],
    scrollTop: 0,
    error: "",
  });
  const [dirty, setDirty] = useState(false);
  const [pending, setPending] = useState<(() => void) | null>(null);
  const go = (action: () => void) => {
    if (dirty) setPending(() => action);
    else action();
  };
  const navigate = (destination: Page) =>
    go(() => {
      setPage(destination);
    });
  return (
    <PlayerHistoryProvider
      clientKey={`${live.workspace?.status.state}:${live.workspace?.status.account?.id}`}
      onOpen={(target) =>
        go(() => {
          historyView.current = {
            server: target.server,
            riotId: target.riotId,
            request: target,
            selected: [],
            scrollTop: 0,
            error: "",
          };
          setHistoryNavigation((n) => n + 1);
          setPage("search");
        })
      }
    >
      <div
        className="app-shell"
        data-compact-sidebar={compactSidebar || undefined}
      >
        <Toaster toasterId="feedback" position="bottom-end" />
        <DesktopLifecycle
          unsaved={dirty || live.saving || live.hasPending || live.busy}
        />
        <ConflictDialog store={live} />
        <aside className="sidebar">
          <div className="brand">
            <svg
              width="32"
              height="36"
              viewBox="0 0 32 36"
              fill="none"
              aria-hidden="true"
            >
              <path d="M5 5H13V24H27L22 31H5V5Z" fill="currentColor" />
              <path
                d="M19 5L28 12L19 19V5Z"
                fill="currentColor"
                opacity=".65"
              />
            </svg>
            <div>
              <strong>LeagueReplay</strong>
              <small>{ui("留住每一场")}</small>
            </div>
          </div>
          <div className="nav-caption">{ui("工作区")}</div>
          <nav aria-label={ui("主导航")}>
            {navigation.map((item) => (
              <Tooltip
                key={item.id}
                content={ui(item.label)}
                relationship="label"
                visible={compactSidebar && navTooltip === item.id}
                onVisibleChange={(_, data) =>
                  setNavTooltip((current) =>
                    data.visible && compactSidebar
                      ? item.id
                      : current === item.id
                        ? null
                        : current,
                  )
                }
              >
                <Button
                  className={`nav-button ${page === item.id ? "active" : ""}`}
                  appearance="subtle"
                  icon={item.icon}
                  aria-label={ui(item.label)}
                  onClick={() => navigate(item.id)}
                  aria-current={page === item.id ? "page" : undefined}
                >
                  <span className="nav-text">{ui(item.label)}</span>
                </Button>
              </Tooltip>
            ))}
          </nav>
          <div className="sidebar-bottom">
            <Tooltip
              content={ui("设置")}
              relationship="label"
              visible={compactSidebar && navTooltip === "settings"}
              onVisibleChange={(_, data) =>
                setNavTooltip((current) =>
                  data.visible && compactSidebar
                    ? "settings"
                    : current === "settings"
                      ? null
                      : current,
                )
              }
            >
              <Button
                className={`nav-button ${page === "settings" ? "active" : ""}`}
                appearance="subtle"
                icon={<Settings24Regular />}
                aria-label={ui("设置")}
                aria-current={page === "settings" ? "page" : undefined}
                onClick={() => navigate("settings")}
              >
                <span className="nav-text">{ui("设置")}</span>
              </Button>
            </Tooltip>
            <div className="sidebar-version">
              0.2.0 <span>PREVIEW</span>
            </div>
          </div>
        </aside>
        <div className="workspace">
          <header className="topbar">
            <div>
              <span>{ui("工作区")}</span>
              <ChevronRight20Regular />
              <strong>
                {page === "settings"
                  ? ui("设置")
                  : ui(
                      navigation.find((item) => item.id === page)?.label ?? "",
                    )}
              </strong>
            </div>
          </header>
          {live.error && (
            <MessageBar intent="error">
              <MessageBarBody>{explainError(live.error)}</MessageBarBody>
              <MessageBarActions>
                <Button onClick={() => void live.retry()}>
                  {live.hasPending ? ui("重试保存") : ui("刷新状态")}
                </Button>
              </MessageBarActions>
            </MessageBar>
          )}
          <main id="main-content" className="workspace-main">
            <LivePage
              historyView={historyView}
              key={`${page}:${historyNavigation}`}
              page={page}
              store={live}
              onSearch={() => navigate("search")}
              onDirtyChange={setDirty}
              onNavigate={go}
            />
          </main>
          <footer className="statusbar">
            <button onClick={() => navigate("settings")}>
              <CircleSmall20Filled />
              <span>
                {live.workspace
                  ? ui(live.workspace.status.message)
                  : isTauri()
                    ? ui("正在读取客户端状态")
                    : ui("浏览器预览 · 需要桌面版连接客户端")}
              </span>
            </button>
            <div>
              <span aria-live="polite">
                {live.error || live.conflict
                  ? ui("请检查归档状态")
                  : live.saving || live.hasPending
                    ? ui("正在保存标记…")
                    : live.workspace
                      ? ui("本地归档已保存")
                      : ""}
              </span>
            </div>
          </footer>
        </div>
        <Dialog
          open={!!pending}
          onOpenChange={(_, data) => {
            if (!data.open) setPending(null);
          }}
        >
          <DialogSurface>
            <DialogBody>
              <DialogTitle>{ui("笔记还没有保存")}</DialogTitle>
              <DialogContent>
                {ui("继续编辑并保存，或放弃这次草稿后离开。")}
              </DialogContent>
              <DialogActions>
                <Button appearance="primary" onClick={() => setPending(null)}>
                  {ui("继续编辑")}
                </Button>
                <Button
                  onClick={() => {
                    pending?.();
                    setPending(null);
                    setDirty(false);
                  }}
                >
                  {ui("放弃草稿并离开")}
                </Button>
              </DialogActions>
            </DialogBody>
          </DialogSurface>
        </Dialog>
      </div>
    </PlayerHistoryProvider>
  );
}
