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
  Input,
} from "@fluentui/react-components";
import { useI18n, ui } from "../i18n";
import type { PlayerProfile } from "../generated/live/PlayerProfile";
import type { LiveStore } from "./useLiveWorkspace";
export function ProfileTools({
  profile,
  store,
}: {
  profile: PlayerProfile;
  store: LiveStore;
}) {
  const { t } = useI18n();
  const [editing, setEditing] = useState(false);
  const [removing, setRemoving] = useState(false);
  const [name, setName] = useState(profile.name);
  const save = () => {
    if (store.saving || store.busy || (!removing && !name.trim())) return;
    store.update((current) => ({
      ...current,
      identityLinks: removing
        ? (current.identityLinks ?? []).filter(
            (link) => link.playerId !== profile.id,
          )
        : current.identityLinks,
      players: removing
        ? current.players.filter((player) => player.id !== profile.id)
        : current.players.map((player) =>
            player.id === profile.id
              ? { ...player, name: name.trim() }
              : player,
          ),
    }));
    setEditing(false);
    setRemoving(false);
  };
  return (
    <>
      <Button
        appearance="subtle"
        disabled={store.saving || store.busy}
        onClick={() => {
          setName(profile.name);
          setEditing(true);
        }}
      >
        {t("profileEdit")}
      </Button>
      <Dialog
        open={editing}
        onOpenChange={(_, d) => {
          if (!d.open) {
            setEditing(false);
            setRemoving(false);
          }
        }}
      >
        <DialogSurface>
          <DialogBody>
            <DialogTitle>
              {t(removing ? "profileRemove" : "profileEdit")}
            </DialogTitle>
            <DialogContent>
              <div className="live-form">
                {removing ? (
                  <p>{t("profileRemoveHint", { name: profile.name })}</p>
                ) : (
                  <>
                    <Field label={ui("你熟悉的称呼")}>
                      <Input
                        value={name}
                        maxLength={80}
                        onChange={(_, d) => setName(d.value)}
                        onKeyDown={(event) => {
                          if (
                            event.key === "Enter" &&
                            !event.nativeEvent.isComposing
                          ) {
                            event.preventDefault();
                            save();
                          }
                        }}
                      />
                    </Field>
                    <Button onClick={() => setRemoving(true)}>
                      {t("profileRemove")}
                    </Button>
                  </>
                )}
              </div>
            </DialogContent>
            <DialogActions>
              <Button
                onClick={() => {
                  if (removing) setRemoving(false);
                  else setEditing(false);
                }}
              >
                {t("cancel")}
              </Button>
              <Button
                appearance="primary"
                disabled={
                  store.saving || store.busy || (!removing && !name.trim())
                }
                onClick={save}
              >
                {t(removing ? "profileRemove" : "sessionConfirm")}
              </Button>
            </DialogActions>
          </DialogBody>
        </DialogSurface>
      </Dialog>
    </>
  );
}
