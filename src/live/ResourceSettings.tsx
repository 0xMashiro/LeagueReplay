import { useState } from "react";
import { invoke } from "../ipc";
import { Button, MessageBar } from "@fluentui/react-components";
import { useI18n } from "../i18n";
import { applyResources, useResources } from "../resources";
export function ResourceSettings() {
  const { t, error: explain } = useI18n();
  const resources = useResources();
  const [busy, setBusy] = useState(false),
    [error, setError] = useState("");
  return (
    <section>
      <h2>{t("gameResources")}</h2>
      <div className="setting-row">
        <div>
          <strong>
            {t("resourceVersion", { version: resources.catalog.version })}
          </strong>
          <p>{t("resourceHint")}</p>
        </div>
        <Button
          disabled={busy}
          onClick={() => {
            setBusy(true);
            setError("");
            void invoke("update_resources")
              .then(applyResources)
              .catch((e) => setError(String(e)))
              .finally(() => setBusy(false));
          }}
        >
          {t("updateResources")}
        </Button>
      </div>
      {error && <MessageBar intent="error">{explain(error)}</MessageBar>}
    </section>
  );
}
