import { useEffect, useState } from "react";
import { invoke } from "../ipc";
import { Button, MessageBar } from "@fluentui/react-components";
import { useI18n } from "../i18n";
export function ReplaySettings() {
  const { t, error: explain } = useI18n();
  const [directory, setDirectory] = useState("");
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  useEffect(() => {
    void invoke("replay_directory")
      .then(setDirectory)
      .catch((e) => setError(String(e)));
  }, []);
  return (
    <section>
      <h2>{t("replayDirectory")}</h2>
      <div className="setting-row">
        <div>
          <strong className="backup-path">{directory}</strong>
          <p>{t("directoryHint")}</p>
        </div>
        <Button
          disabled={busy}
          onClick={() => {
            setBusy(true);
            setError("");
            void invoke("choose_replay_directory")
              .then((path) => {
                if (path) setDirectory(path);
              })
              .catch((e) => setError(String(e)))
              .finally(() => setBusy(false));
          }}
        >
          {t("chooseDirectory")}
        </Button>
      </div>
      {error && <MessageBar intent="error">{explain(error)}</MessageBar>}
    </section>
  );
}
