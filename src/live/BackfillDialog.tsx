import { useState } from "react";
import {
  Button,
  Dialog,
  DialogActions,
  DialogBody,
  DialogContent,
  DialogSurface,
  DialogTitle,
  Field,
  MessageBar,
} from "@fluentui/react-components";
import type { ReviewGame } from "../generated/library/ReviewGame";
import type { LiveStore } from "./useLiveWorkspace";
import { useI18n } from "../i18n";
import { SessionSelect } from "./SessionSelect";

export function BackfillDialog({
  game,
  store,
  onClose,
}: {
  game: ReviewGame;
  store: LiveStore;
  onClose: () => void;
}) {
  const [session, setSession] = useState("new");
  const { t, error } = useI18n();
  const sessions = store.workspace?.sessions ?? [];
  const existing = store.workspace?.matches.find(
    (m) => m.id === game.id && m.accountId === game.account.id,
  );
  const attributed = sessions.find((s) =>
    s.segments.some((segment) => segment.id === existing?.segmentId),
  );
  const destination = attributed?.id ?? session;
  const selected = sessions.find((s) => s.id === destination);
  return (
    <Dialog
      open
      onOpenChange={(_, data) => {
        if (!data.open && !store.busy) onClose();
      }}
    >
      <DialogSurface>
        <DialogBody>
          <DialogTitle>{t("addTitle")}</DialogTitle>
          <DialogContent>
            <div className="live-form">
              {store.error && (
                <MessageBar intent="error">{error(store.error)}</MessageBar>
              )}
              <p>
                {t("addHint", {
                  name: `${game.account.riotId} · ${game.account.platform}`,
                })}
              </p>
              <p className="muted">{t("addNote")}</p>
              <Field label={t("session")}>
                <SessionSelect
                  sessions={sessions}
                  value={destination}
                  onChange={setSession}
                  allowNew
                  disabled={store.busy || !!attributed}
                />
              </Field>
              <p className="session-operation-summary">
                {attributed
                  ? t("backfillAlreadyHint", { title: attributed.title })
                  : selected
                    ? t("backfillExistingHint", {
                        title: selected.title,
                        count:
                          selected.segments.reduce(
                            (n, s) => n + s.matchIds.length,
                            0,
                          ) + 1,
                      })
                    : t("backfillNewHint")}
              </p>
            </div>
          </DialogContent>
          <DialogActions>
            <Button disabled={store.busy} onClick={onClose}>
              {t("cancel")}
            </Button>
            <Button
              appearance="primary"
              disabled={store.busy || (destination !== "new" && !selected)}
              onClick={() =>
                void store
                  .command([
                    "backfill_review_game",
                    {
                      id: game.id,
                      puuid: game.account.puuid,
                      sessionId: destination === "new" ? null : destination,
                    },
                  ])
                  .then((ok) => {
                    if (ok) onClose();
                  })
              }
            >
              {t("confirmAdd")}
            </Button>
          </DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
}
