import { ui, useI18n } from "../i18n";
import { useState } from "react";
import {
  Button,
  Checkbox,
  Dialog,
  DialogActions,
  DialogBody,
  DialogContent,
  DialogSurface,
  DialogTitle,
  Dropdown,
  Field,
  Input,
  Option,
  MessageBar,
} from "@fluentui/react-components";
import type { ObservedMatch } from "../generated/live/ObservedMatch";
import type { LiveStore } from "./useLiveWorkspace";
import { participantAccountKey } from "./adapter";
import { playerForAccount } from "./identity";

export function PlayerPanel({
  store,
  record,
  onClose,
}: {
  store: LiveStore;
  record: ObservedMatch;
  onClose: () => void;
}) {
  const { t } = useI18n();
  const labels = store.workspace!.ui;
  const identities = (record.game?.participants ?? []).filter((p) =>
    participantAccountKey(record.platform, p),
  );
  const initialAccount =
    participantAccountKey(record.platform, identities[0]) ?? "";
  const at = record.game?.startedAt || record.firstSeen;
  const day = new Date(at);
  const initialDate = [
    day.getFullYear(),
    String(day.getMonth() + 1).padStart(2, "0"),
    String(day.getDate()).padStart(2, "0"),
  ].join("-");
  const [account, setAccount] = useState(initialAccount);
  const [profile, setProfile] = useState(
    playerForAccount(labels, initialAccount, record.id, at)?.id ?? "new",
  );
  const [name, setName] = useState("");
  const [scope, setScope] = useState("match");
  const [from, setFrom] = useState(initialDate);
  const [to, setTo] = useState(initialDate);
  const [premade, setPremade] = useState(
    labels.premades.some(
      (p) => p.matchId === record.id && p.accountId === initialAccount,
    ),
  );
  const chosen = identities.find(
    (p) => participantAccountKey(record.platform, p) === account,
  );
  const ownIdentity = identities.find(
    (p) => participantAccountKey(record.platform, p) === record.accountId,
  );
  const sameTeam =
    account !== record.accountId &&
    !!ownIdentity?.team &&
    chosen?.team === ownIdentity.team;
  const start = Date.parse(from + "T00:00:00"),
    end = Date.parse(to + "T23:59:59.999");
  const invalid =
    scope === "period" &&
    (!Number.isFinite(start) || !Number.isFinite(end) || start > end);
  const overlap =
    scope === "period" &&
    !invalid &&
    (labels.identityLinks ?? []).some(
      (l) =>
        l.accountId === account &&
        !l.matchId &&
        l.from! <= end &&
        start <= l.to! &&
        !(l.from === start && l.to === end),
    );
  const scopeLabel =
    scope === "match"
      ? "identityMatch"
      : scope === "period"
        ? "identityPeriod"
        : "identityAccount";
  const save = () => {
    store.update((state) => {
      const playerId = profile === "new" ? crypto.randomUUID() : profile;
      let players =
        profile === "new"
          ? [
              ...state.players,
              { id: playerId, name: name.trim(), accountIds: [] as string[] },
            ]
          : state.players;
      let links = state.identityLinks ?? [];
      if (scope === "account")
        players = players.map((p) => ({
          ...p,
          accountIds: [
            ...p.accountIds.filter((a) => a !== account),
            ...(p.id === playerId ? [account] : []),
          ],
        }));
      else {
        links = links.filter(
          (l) =>
            !(
              l.accountId === account &&
              (scope === "match"
                ? l.matchId === record.id
                : !l.matchId && l.from === start && l.to === end)
            ),
        );
        links = [
          ...links,
          {
            id: crypto.randomUUID(),
            playerId,
            accountId: account,
            matchId: scope === "match" ? record.id : null,
            from: scope === "period" ? start : null,
            to: scope === "period" ? end : null,
          },
        ];
      }
      const premades = state.premades.filter(
        (p) => !(p.matchId === record.id && p.accountId === account),
      );
      return {
        ...state,
        players,
        identityLinks: links,
        premades:
          sameTeam && premade
            ? [...premades, { matchId: record.id, accountId: account }]
            : premades,
      };
    });
    onClose();
  };
  return (
    <Dialog
      open
      onOpenChange={(_, d) => {
        if (!d.open) onClose();
      }}
    >
      <DialogSurface>
        <DialogBody>
          <DialogTitle>{ui("标记玩家与组队关系")}</DialogTitle>
          <DialogContent>
            <div className="live-form">
              <Field label={ui("这场对局中的账号")}>
                <Dropdown
                  value={chosen?.name || ui("未知玩家")}
                  selectedOptions={[account]}
                  onOptionSelect={(_, d) => {
                    const account = d.optionValue!;
                    setAccount(account);
                    setProfile(
                      playerForAccount(labels, account, record.id, at)?.id ??
                        "new",
                    );
                    setName("");
                    setPremade(
                      labels.premades.some(
                        (p) =>
                          p.matchId === record.id && p.accountId === account,
                      ),
                    );
                  }}
                >
                  {identities.map((p) => (
                    <Option
                      key={participantAccountKey(record.platform, p)}
                      value={participantAccountKey(record.platform, p)!}
                    >
                      {p.name || ui("未知玩家")}
                    </Option>
                  ))}
                </Dropdown>
              </Field>
              <Field label={ui("关联玩家档案")}>
                <Dropdown
                  value={
                    profile === "new"
                      ? ui("新建玩家")
                      : labels.players.find((p) => p.id === profile)?.name
                  }
                  selectedOptions={[profile]}
                  onOptionSelect={(_, d) => setProfile(d.optionValue!)}
                >
                  <Option value="new">{ui("新建玩家")}</Option>
                  {labels.players.map((p) => (
                    <Option key={p.id} value={p.id}>
                      {p.name}
                    </Option>
                  ))}
                </Dropdown>
              </Field>
              {profile === "new" && (
                <Field label={ui("你熟悉的称呼")}>
                  <Input
                    value={name}
                    maxLength={80}
                    onChange={(_, d) => setName(d.value)}
                    placeholder={ui("例如：XX")}
                  />
                </Field>
              )}
              <Field label={t("identityScope")}>
                <Dropdown
                  value={t(scopeLabel)}
                  selectedOptions={[scope]}
                  onOptionSelect={(_, d) => setScope(d.optionValue!)}
                >
                  <Option value="match">{t("identityMatch")}</Option>
                  <Option value="period">{t("identityPeriod")}</Option>
                  <Option value="account">{t("identityAccount")}</Option>
                </Dropdown>
              </Field>
              {scope === "period" && (
                <div className="identity-dates">
                  <Field label={t("identityFrom")}>
                    <Input
                      type="date"
                      value={from}
                      onChange={(_, d) => setFrom(d.value)}
                    />
                  </Field>
                  <Field label={t("identityTo")}>
                    <Input
                      type="date"
                      value={to}
                      onChange={(_, d) => setTo(d.value)}
                    />
                  </Field>
                </div>
              )}
              <p className="muted">{t("identityHint")}</p>
              {(invalid || overlap) && (
                <MessageBar intent="warning">
                  {t(overlap ? "identityOverlap" : "identityInvalid")}
                </MessageBar>
              )}
              <Checkbox
                disabled={!sameTeam}
                checked={sameTeam && premade}
                onChange={(_, d) => setPremade(d.checked === true)}
                label={ui("确认这局是一起组队的")}
              />
            </div>
          </DialogContent>
          <DialogActions>
            <Button onClick={onClose}>{ui("取消")}</Button>
            <Button
              appearance="primary"
              disabled={
                !account ||
                invalid ||
                overlap ||
                (profile === "new" && !name.trim()) ||
                store.saving ||
                !!store.conflict
              }
              onClick={save}
            >
              {ui("保存标记")}
            </Button>
          </DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
}
