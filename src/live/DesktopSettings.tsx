import { useEffect, useState } from "react";
import { invoke } from "../ipc";
import { Switch, MessageBar, Spinner } from "@fluentui/react-components";
import { useI18n } from "../i18n";
import type { Settings } from "../generated/desktop/Settings";
export function DesktopSettings() {
  const { t, language } = useI18n();
  const [settings, setSettings] = useState<Settings>();
  const [error, setError] = useState(false);
  const [busy, setBusy] = useState(false);
  useEffect(() => {
    void invoke("desktop_settings")
      .then(setSettings)
      .catch(() => setError(true));
  }, []);
  const save = async (kind: "tray" | "autostart", checked: boolean) => {
    if (!settings || busy) return;
    setBusy(true);
    setError(false);
    try {
      if (kind === "tray")
        await invoke("save_desktop_settings", {
          preferences: { closeToTray: checked, language },
        });
      else await invoke("set_autostart", { enabled: checked });
      setSettings(await invoke("desktop_settings"));
    } catch {
      setError(true);
    } finally {
      setBusy(false);
    }
  };
  return (
    <section>
      <h2>{t("background")}</h2>
      {error && (
        <MessageBar intent="error">{t("desktopSettingsError")}</MessageBar>
      )}
      {!settings && !error && <Spinner size="tiny" />}
      <div className="setting-row">
        <div>
          <strong>{t("closeToTray")}</strong>
          <p>{t("trayHint")}</p>
        </div>
        <Switch
          aria-label={t("closeToTray")}
          checked={settings?.preferences.closeToTray ?? false}
          disabled={!settings || busy}
          onChange={(_, d) => void save("tray", d.checked)}
        />
      </div>
      <div className="setting-row">
        <div>
          <strong>{t("autostart")}</strong>
          <p>{t("autostartHint")}</p>
        </div>
        <Switch
          aria-label={t("autostart")}
          checked={settings?.autostart ?? false}
          disabled={!settings || busy}
          onChange={(_, d) => void save("autostart", d.checked)}
        />
      </div>
    </section>
  );
}
