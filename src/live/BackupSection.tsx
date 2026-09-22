import { useState } from "react";
import { invoke } from "../ipc";
import {
  Button,
  Dialog,
  DialogActions,
  DialogBody,
  DialogContent,
  DialogSurface,
  DialogTitle,
  MessageBar,
  Checkbox,
} from "@fluentui/react-components";
import { useI18n } from "../i18n";
import type { BackupSelection } from "../generated/library/BackupSelection";
import type { RestoreBackupRequest } from "../generated/library/RestoreBackupRequest";
import type { LiveStore } from "./useLiveWorkspace";

export function BackupSection({ store }: { store: LiveStore }) {
  const { t, date, error: explain } = useI18n();
  const [merge, setMerge] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [saved, setSaved] = useState("");
  const [candidate, setCandidate] = useState<BackupSelection>();
  const blocked = busy || store.busy || store.saving || store.hasPending;
  return (
    <section>
      <h2>{t("backupTitle")}</h2>
      <div className="setting-row backup-row">
        <div>
          <strong>{t("backupAll")}</strong>
          <p>{t("backupHint")}</p>
        </div>
        <div className="backup-actions">
          <Button
            disabled={blocked}
            onClick={() => {
              setBusy(true);
              setError("");
              setSaved("");
              void invoke("export_backup")
                .then((path) => {
                  if (path) setSaved(path);
                })
                .catch((e) => setError(String(e)))
                .finally(() => setBusy(false));
            }}
          >
            {t("backupExport")}
          </Button>
          <Button
            disabled={blocked}
            onClick={() => {
              setBusy(true);
              setError("");
              setSaved("");
              void invoke("export_backup_package")
                .then((path) => {
                  if (path) setSaved(path);
                })
                .catch((e) => setError(String(e)))
                .finally(() => setBusy(false));
            }}
          >
            {t("backupZip")}
          </Button>
          <Button
            disabled={blocked}
            onClick={() => {
              setBusy(true);
              setError("");
              setSaved("");
              void invoke("choose_backup")
                .then((selection) => {
                  if (!selection) return;
                  setCandidate(selection);
                  setMerge(true);
                })
                .catch((e) => setError(String(e)))
                .finally(() => setBusy(false));
            }}
          >
            {t("backupNativeChoose")}
          </Button>
        </div>
      </div>
      {!candidate && error && (
        <MessageBar intent="error">{explain(error)}</MessageBar>
      )}
      {saved && (
        <MessageBar intent="success">
          <div>{t("packageSaved", { path: saved })}</div>
        </MessageBar>
      )}
      <Dialog
        open={!!candidate}
        onOpenChange={(_, d) => {
          if (!d.open && !blocked) setCandidate(undefined);
        }}
      >
        <DialogSurface>
          <DialogBody>
            <DialogTitle>{t("backupRestore")}</DialogTitle>
            <DialogContent>
              <div className="live-form">
                <strong>{candidate?.path.split(/[\\/]/).pop()}</strong>
                {candidate && (
                  <>
                    <p>{date(candidate.preview.createdAt)}</p>
                    <dl className="backup-summary">
                      {(
                        [
                          ["sessions", "backupSessions"],
                          ["participations", "backupPlays"],
                          ["games", "backupGames"],
                          ["subscriptions", "followTitle"],
                          ["replay_files", "backupFiles"],
                          ["notes", "notes"],
                          ["players", "playerProfiles"],
                        ] as const
                      ).map(([key, label]) => (
                        <div key={key}>
                          <dt>{t(label)}</dt>
                          <dd>{candidate.preview.counts[key] ?? 0}</dd>
                        </div>
                      ))}
                    </dl>
                  </>
                )}
                <Checkbox
                  checked={merge}
                  label={t("backupMerge")}
                  onChange={(_, d) => setMerge(d.checked === true)}
                  disabled={blocked}
                />
                <MessageBar intent={merge ? "info" : "warning"}>
                  {t(merge ? "backupMergeHint" : "backupReplaceHint")}
                </MessageBar>
                <p className="muted">{t("backupRestoreHint")}</p>
                {error && (
                  <MessageBar intent="error">{explain(error)}</MessageBar>
                )}
              </div>
            </DialogContent>
            <DialogActions>
              <Button
                disabled={blocked}
                onClick={() => setCandidate(undefined)}
              >
                {t("cancel")}
              </Button>
              <Button
                appearance="primary"
                disabled={blocked || !candidate}
                onClick={() => {
                  if (!candidate) return;
                  setBusy(true);
                  setError("");
                  void store
                    .command(
                      [
                        "restore_backup",
                        {
                          request: {
                            path: candidate.path,
                            sourceFingerprint: candidate.sourceFingerprint,
                            fingerprint: candidate.preview.fingerprint,
                            merge,
                          } satisfies RestoreBackupRequest,
                        },
                      ],
                      (result) => setSaved(String(result)),
                      setError,
                    )
                    .then((ok) => {
                      if (ok) setCandidate(undefined);
                    })
                    .finally(() => setBusy(false));
                }}
              >
                {t(
                  blocked
                    ? "sessionSaving"
                    : merge
                      ? "backupMerge"
                      : "backupRestore",
                )}
              </Button>
            </DialogActions>
          </DialogBody>
        </DialogSurface>
      </Dialog>
    </section>
  );
}
