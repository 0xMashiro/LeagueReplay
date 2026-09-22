import { useState } from "react";
import {
  Button,
  Dialog,
  DialogSurface,
  DialogBody,
  DialogTitle,
  DialogContent,
  DialogActions,
  Dropdown,
  Option,
  Field,
  Input,
} from "@fluentui/react-components";
import { ui, useI18n } from "../i18n";
import type { Account } from "../generated/live/Account";
import type { LiveStore } from "./useLiveWorkspace";

export function AssociatePlayer({
  account,
  store,
}: {
  account: Account;
  store: LiveStore;
}) {
  const { t } = useI18n();
  const [open, setOpen] = useState(false);
  const [selected, setSelected] = useState("new");
  const [name, setName] = useState("");
  const players = store.workspace!.ui.players;
  const current = players.find((p) => p.accountIds.includes(account.id));
  const busy = store.busy || store.saving;
  const valid =
    selected === "new" ? !!name.trim() : players.some((p) => p.id === selected);
  const save = () => {
    if (busy || !valid) return;
    store.update((state) => {
      const id = selected === "new" ? crypto.randomUUID() : selected;
      const profiles =
        selected === "new"
          ? [...state.players, { id, name: name.trim(), accountIds: [] }]
          : state.players;
      return {
        ...state,
        players: profiles.map((p) => ({
          ...p,
          accountIds: [
            ...p.accountIds.filter((a) => a !== account.id),
            ...(p.id === id ? [account.id] : []),
          ],
        })),
      };
    });
    setOpen(false);
  };
  return (
    <>
      <Button
        className="associate-player"
        appearance="subtle"
        disabled={busy}
        onClick={() => {
          setSelected(current?.id ?? "new");
          setName("");
          setOpen(true);
        }}
      >
        {t("associatePlayer")}
        {current ? ` · ${current.name}` : ""}
      </Button>
      <Dialog open={open} onOpenChange={(_, data) => setOpen(data.open)}>
        <DialogSurface>
          <DialogBody>
            <DialogTitle>{t("associatePlayer")}</DialogTitle>
            <DialogContent>
              <div className="live-form">
                <strong>
                  {account.riotId} · {account.platform}
                </strong>
                <Field label={ui("关联玩家档案")}>
                  <Dropdown
                    selectedOptions={[selected]}
                    value={
                      selected === "new"
                        ? ui("新建玩家")
                        : (players.find((p) => p.id === selected)?.name ?? "")
                    }
                    onOptionSelect={(_, d) => setSelected(d.optionValue!)}
                  >
                    <Option value="new">{ui("新建玩家")}</Option>
                    {players.map((p) => (
                      <Option key={p.id} value={p.id}>
                        {p.name}
                      </Option>
                    ))}
                  </Dropdown>
                </Field>
                {selected === "new" && (
                  <Field label={ui("你熟悉的称呼")}>
                    <Input
                      value={name}
                      maxLength={80}
                      onChange={(_, d) => setName(d.value)}
                    />
                  </Field>
                )}
                <p className="muted">{t("associatePlayerScope")}</p>
              </div>
            </DialogContent>
            <DialogActions>
              <Button onClick={() => setOpen(false)}>{ui("取消")}</Button>
              <Button
                appearance="primary"
                disabled={busy || !valid}
                onClick={save}
              >
                {ui("保存")}
              </Button>
            </DialogActions>
          </DialogBody>
        </DialogSurface>
      </Dialog>
    </>
  );
}
