import { useEffect, useState } from "react";
import { isTauri } from "@tauri-apps/api/core";
import { invoke } from "../ipc";
import { listen } from "@tauri-apps/api/event";
import {
  Button,
  Dialog,
  DialogSurface,
  DialogBody,
  DialogTitle,
  DialogContent,
  DialogActions,
} from "@fluentui/react-components";
import { ui, useI18n } from "../i18n";
export function DesktopLifecycle({ unsaved }: { unsaved: boolean }) {
  const [open, setOpen] = useState(false);
  const { t, language } = useI18n();
  useEffect(() => {
    if (!isTauri()) return;
    const unlisten = listen("desktop-request-exit", () => setOpen(true));
    void unlisten.then(() => invoke("desktop_ready")).catch(() => {});
    return () => {
      void unlisten.then((off) => off());
    };
  }, []);
  useEffect(() => {
    if (!isTauri()) return;
    void invoke("desktop_settings")
      .then(({ preferences }) => {
        if (preferences.language !== language)
          return invoke("save_desktop_settings", {
            preferences: { ...preferences, language },
          });
      })
      .catch(() => {});
  }, [language]);
  return (
    <Dialog open={open} onOpenChange={(_, data) => setOpen(data.open)}>
      <DialogSurface>
        <DialogBody>
          <DialogTitle>{t("quitApp")}</DialogTitle>
          <DialogContent>
            {t(unsaved ? "quitUnsaved" : "quitHint")}
          </DialogContent>
          <DialogActions>
            <Button appearance="primary" onClick={() => setOpen(false)}>
              {ui("取消")}
            </Button>
            <Button
              disabled={unsaved}
              onClick={() => void invoke("exit_desktop")}
            >
              {t("quitApp")}
            </Button>
          </DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
}
