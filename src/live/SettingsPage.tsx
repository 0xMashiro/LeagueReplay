import { Badge, Dropdown, Option } from "@fluentui/react-components";
import { ui, languages, setLanguage, useI18n, type Language } from "../i18n";
import { gameLabel } from "../game-labels";
import {
  useThemePreference,
  setThemePreference,
  type ThemePreference,
} from "../theme-preference";
import type { LiveWorkspace } from "../generated/live/LiveWorkspace";
import type { LiveStore } from "./useLiveWorkspace";
import { BackupSection } from "./BackupSection";
import { ResourceSettings } from "./ResourceSettings";
import { ReplaySettings } from "./ReplaySettings";
import { DesktopSettings } from "./DesktopSettings";

export function SettingsPage({
  view,
  store,
}: {
  view: LiveWorkspace;
  store: LiveStore;
}) {
  const theme = useThemePreference();
  const { t, language } = useI18n();
  return (
    <div className="page-scroll live-page settings-page">
      <header className="live-heading">
        <div>
          <h1>{ui("设置")}</h1>
        </div>
      </header>
      <div className="settings-content">
        <section>
          <h2>{ui("游戏客户端")}</h2>
          <div className="setting-row">
            <div>
              <strong>{ui(view.status.message)}</strong>
              <p>
                {view.status.account && (
                  <>
                    {view.status.account.riotId} ·{" "}
                    {view.status.account.platform}
                    <br />
                  </>
                )}
                {ui(
                  "空闲 {minutes} 分钟自动结束场次；切号和跨午夜不直接拆分。",
                  { minutes: view.idleMinutes },
                )}
              </p>
            </div>
            <Badge appearance="outline">
              {view.status.phase ? gameLabel(view.status.phase) : ui("未连接")}
            </Badge>
          </div>
        </section>
        <section>
          <h2>{ui("外观")}</h2>
          <div className="setting-row">
            <span>{ui("界面主题")}</span>
            <Dropdown
              aria-label={ui("界面主题")}
              value={
                {
                  dark: ui("深色"),
                  light: ui("浅色"),
                  system: ui("跟随系统"),
                }[theme]
              }
              selectedOptions={[theme]}
              onOptionSelect={(_, data) =>
                setThemePreference(data.optionValue as ThemePreference)
              }
            >
              <Option value="dark">{ui("深色")}</Option>
              <Option value="light">{ui("浅色")}</Option>
              <Option value="system">{ui("跟随系统")}</Option>
            </Dropdown>
          </div>
        </section>
        <section>
          <h2>{t("language")}</h2>
          <div className="setting-row">
            <p className="muted">{t("languageHint")}</p>
            <Dropdown
              aria-label={t("language")}
              value={languages[language]}
              selectedOptions={[language]}
              onOptionSelect={(_, data) =>
                setLanguage(data.optionValue as Language)
              }
            >
              {Object.entries(languages).map(([id, name]) => (
                <Option key={id} value={id}>
                  {name}
                </Option>
              ))}
            </Dropdown>
          </div>
        </section>
        <DesktopSettings />
        <ReplaySettings />
        <ResourceSettings />
        <BackupSection store={store} />
        <section>
          <h2>{ui("当前版本")} · 0.2.0</h2>
          <p>{t("updatesUnconfigured")}</p>
        </section>
      </div>
    </div>
  );
}
