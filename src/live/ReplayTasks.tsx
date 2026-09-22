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
  ProgressBar,
  Menu,
  MenuTrigger,
  MenuPopover,
  MenuList,
  MenuItem,
  MenuDivider,
  Tooltip,
} from "@fluentui/react-components";
import {
  ArrowDownload20Regular,
  CheckmarkCircle20Regular,
  MoreHorizontal20Regular,
  Play20Regular,
} from "@fluentui/react-icons";
import type { ReplayJob } from "../generated/library/ReplayJob";
import { useI18n } from "../i18n";

export function RemoveReplayDialog({
  id,
  onClose,
  onRemoved,
}: {
  id: string;
  onClose: () => void;
  onRemoved?: () => void;
}) {
  const { t, error: explain } = useI18n();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  return (
    <Dialog
      open
      onOpenChange={(_, d) => {
        if (!d.open && !busy) onClose();
      }}
    >
      <DialogSurface>
        <DialogBody>
          <DialogTitle>{t("removeReplay")}</DialogTitle>
          <DialogContent>
            <div className="live-form">
              {error && (
                <MessageBar intent="error">{explain(error)}</MessageBar>
              )}
              <p>{t("removeReplayHint", { id })}</p>
            </div>
          </DialogContent>
          <DialogActions>
            <Button disabled={busy} onClick={onClose}>
              {t("cancel")}
            </Button>
            <Button
              appearance="primary"
              disabled={busy}
              onClick={() => {
                setBusy(true);
                setError("");
                void invoke("remove_replay", { id })
                  .then(() => {
                    onRemoved?.();
                    onClose();
                  })
                  .catch((e) => setError(String(e)))
                  .finally(() => setBusy(false));
              }}
            >
              {t("removeReplay")}
            </Button>
          </DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
}

const stateLabels = {
  waiting: "waitingClient",
  queued: "queued",
  downloading: "downloading",
  validating: "validating",
  ready: "downloaded",
  missing: "missing",
  error: "downloadFailed",
  cancelled: "cancelled",
  cancelling: "cancelling",
} as const;
export function ReplayTasks({
  jobs,
  onReview,
}: {
  jobs: ReplayJob[];
  onReview: (id: string) => void;
}) {
  const { t, error: explain } = useI18n();
  const [busy, setBusy] = useState("");
  const [error, setError] = useState("");
  const [removing, setRemoving] = useState("");
  const command = async (
    name: "cancel_replay" | "download_replay" | "open_replay" | "reveal_replay",
    id: string,
  ) => {
    setBusy(id);
    setError("");
    try {
      await invoke(name, { id });
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy("");
    }
  };
  return (
    <div>
      {error && <MessageBar intent="error">{explain(error)}</MessageBar>}
      {jobs.map((job) => (
        <article className="replay-task" key={job.id} data-state={job.state}>
          <span className="replay-task-icon" aria-hidden="true">
            {job.state === "ready" ? (
              <CheckmarkCircle20Regular />
            ) : (
              <ArrowDownload20Regular />
            )}
          </span>
          <div className="replay-task-info">
            <strong>{job.id}</strong>
            <span className="muted">
              {t(
                stateLabels[job.state as keyof typeof stateLabels] ??
                  "downloadFailed",
              )}{" "}
              · {(job.received / 1048576).toFixed(1)} MB
              {job.total ? ` / ${(job.total / 1048576).toFixed(1)} MB` : ""}
              {job.version ? ` · ${job.version}` : ""}
            </span>
            {["queued", "downloading", "validating", "cancelling"].includes(
              job.state,
            ) && (
              <ProgressBar
                aria-label={t("downloading")}
                value={job.total ? job.received / job.total : undefined}
              />
            )}
            {job.error && (
              <small className="loss-text">{explain(job.error)}</small>
            )}
          </div>
          <div className="live-row-actions">
            <Button
              data-return-key={job.id}
              disabled={!!busy}
              onClick={() => onReview(job.id)}
            >
              {t("review")}
            </Button>
            {["waiting", "queued", "downloading"].includes(job.state) && (
              <Button
                disabled={!!busy}
                onClick={() => void command("cancel_replay", job.id)}
              >
                {t("cancelDownload")}
              </Button>
            )}
            {["error", "cancelled", "missing"].includes(job.state) && (
              <Button
                disabled={!!busy}
                onClick={() => void command("download_replay", job.id)}
              >
                {t("retry")}
              </Button>
            )}
            {job.state === "ready" && (
              <Button
                appearance="primary"
                icon={<Play20Regular />}
                disabled={!!busy}
                onClick={() => void command("open_replay", job.id)}
              >
                {t("playReplay")}
              </Button>
            )}
            {["ready", "missing"].includes(job.state) && (
              <Menu>
                <MenuTrigger disableButtonEnhancement>
                  <Tooltip content={t("moreMatchActions")} relationship="label">
                    <Button
                      appearance="subtle"
                      icon={<MoreHorizontal20Regular />}
                      aria-label={t("moreMatchActions")}
                      disabled={!!busy}
                    />
                  </Tooltip>
                </MenuTrigger>
                <MenuPopover>
                  <MenuList>
                    {job.state === "ready" && (
                      <>
                        <MenuItem
                          onClick={() => void command("reveal_replay", job.id)}
                        >
                          {t("reveal")}
                        </MenuItem>
                        <MenuDivider />
                      </>
                    )}
                    <MenuItem onClick={() => setRemoving(job.id)}>
                      {t("removeReplay")}
                    </MenuItem>
                  </MenuList>
                </MenuPopover>
              </Menu>
            )}
          </div>
        </article>
      ))}
      {!jobs.length && (
        <div className="empty-state">
          <h3>{t("noDownloads")}</h3>
          <p>{t("noDownloadsHint")}</p>
        </div>
      )}
      {removing && (
        <RemoveReplayDialog id={removing} onClose={() => setRemoving("")} />
      )}
    </div>
  );
}
