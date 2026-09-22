import type { Call } from "../ipc";
import { useState } from "react";
import {
  Button,
  Dialog,
  DialogSurface,
  DialogBody,
  DialogTitle,
  DialogContent,
  DialogActions,
  Field,
  Input,
  MessageBar,
} from "@fluentui/react-components";
import { SessionSelect } from "./SessionSelect";
import { useI18n } from "../i18n";
import type { LiveStore } from "./useLiveWorkspace";
import type { SearchResult } from "../generated/library/SearchResult";
export function OrganizeMatchesDialog({
  store,
  ids,
  backfill,
  onClose,
}: {
  store: LiveStore;
  ids: string[];
  backfill?: SearchResult;
  onClose: (completed?: boolean) => void;
}) {
  const { t, error } = useI18n();
  const [destination, setDestination] = useState("new");
  const [title, setTitle] = useState(
    t(backfill ? "newSession" : "splitDefault"),
  );
  const [failure, setFailure] = useState("");
  return (
    <Dialog
      open
      onOpenChange={(_, d) => {
        if (!d.open && !store.busy) onClose();
      }}
    >
      <DialogSurface>
        <DialogBody>
          <DialogTitle>
            {t(backfill ? "batchBackfill" : "moveMatches")}
          </DialogTitle>
          <DialogContent>
            <div className="live-form">
              {failure && (
                <MessageBar intent="error">{error(failure)}</MessageBar>
              )}
              <p>
                {backfill
                  ? t("batchOwner", {
                      name: backfill.account.riotId,
                      count: ids.length,
                    })
                  : t("moveCount", { count: ids.length })}
              </p>
              <Field label={t("session")}>
                <SessionSelect
                  sessions={store.workspace?.sessions ?? []}
                  value={destination}
                  onChange={setDestination}
                  allowNew
                  newLabel={backfill ? undefined : t("createSession")}
                  disabled={store.busy}
                />
              </Field>
              {destination === "new" && (
                <Field label={t("sessionName")}>
                  <Input
                    value={title}
                    disabled={store.busy}
                    maxLength={80}
                    onChange={(_, d) => setTitle(d.value)}
                  />
                </Field>
              )}
              <p className="muted">{t("sessionKeepFacts")}</p>
            </div>
          </DialogContent>
          <DialogActions>
            <Button disabled={store.busy} onClick={() => onClose()}>
              {t("cancel")}
            </Button>
            <Button
              appearance="primary"
              disabled={store.busy || (destination === "new" && !title.trim())}
              onClick={() => {
                setFailure("");
                const call: Call<"backfill_games" | "move_live_matches"> =
                  backfill
                    ? [
                        "backfill_games",
                        {
                          server: backfill.server,
                          puuid: backfill.account.puuid,
                          gameIds: ids,
                          sessionId: destination === "new" ? null : destination,
                          title,
                        },
                      ]
                    : [
                        "move_live_matches",
                        {
                          observationIds: ids,
                          destinationId:
                            destination === "new" ? null : destination,
                          title,
                        },
                      ];
                void store.command(call, undefined, setFailure).then((ok) => {
                  if (ok) onClose(true);
                });
              }}
            >
              {t(backfill ? "confirmAdd" : "sessionConfirm")}
            </Button>
          </DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
}
