import type { ReactNode } from "react";
import {
  Button,
  Badge,
  Dropdown,
  Field,
  Input,
  Option,
  Popover,
  PopoverSurface,
  PopoverTrigger,
} from "@fluentui/react-components";
import { Filter20Regular } from "@fluentui/react-icons";
import { SearchField } from "../components/SearchField";
import { useI18n } from "../i18n";
import type { Account } from "../generated/live/Account";
import { initialPlayFilters, type PlayFilter } from "./play-query";
export function PlayFilters({
  value,
  onChange,
  accounts,
  playerFilters,
  playerFilterActive,
  onResetPlayer,
}: {
  value: PlayFilter;
  onChange: (v: PlayFilter) => void;
  accounts: Account[];
  playerFilters: ReactNode;
  playerFilterActive: boolean;
  onResetPlayer: () => void;
}) {
  const { t } = useI18n();
  const set = (key: keyof PlayFilter, text: string) =>
    onChange({ ...value, [key]: text });
  const activeCount =
    Number(playerFilterActive) +
    Number(value.result !== "all") +
    Number(value.source !== "all") +
    Number(!!value.from) +
    Number(!!value.to);
  return (
    <div className="play-filters">
      <SearchField
        label={t("playSearch")}
        value={value.query}
        onChange={(query) => set("query", query)}
      />
      <Dropdown
        aria-label={t("allAccounts")}
        selectedOptions={[value.account]}
        value={
          accounts.find((a) => a.id === value.account)?.riotId ??
          t("allAccounts")
        }
        onOptionSelect={(_, d) => set("account", d.optionValue!)}
      >
        <Option value="all">{t("allAccounts")}</Option>
        {accounts.map((a) => (
          <Option key={a.id} value={a.id} text={a.riotId}>
            {a.riotId} · {a.platform}
          </Option>
        ))}
      </Dropdown>
      <Popover>
        <PopoverTrigger disableButtonEnhancement>
          <Button
            icon={<Filter20Regular />}
            appearance={activeCount ? "primary" : "secondary"}
          >
            {t("playFilters")}
            {activeCount > 0 && (
              <Badge appearance="filled" color="subtle" size="small">
                {activeCount}
              </Badge>
            )}
          </Button>
        </PopoverTrigger>
        <PopoverSurface>
          <div className="live-form play-filter-panel">
            <Field label={t("matchResult")}>
              <Dropdown
                selectedOptions={[value.result]}
                value={t(
                  value.result === "win"
                    ? "win"
                    : value.result === "loss"
                      ? "loss"
                      : "all",
                )}
                onOptionSelect={(_, d) => set("result", d.optionValue!)}
              >
                <Option value="all">{t("all")}</Option>
                <Option value="win">{t("win")}</Option>
                <Option value="loss">{t("loss")}</Option>
              </Dropdown>
            </Field>
            <Field label={t("recordSource")}>
              <Dropdown
                selectedOptions={[value.source]}
                value={t(
                  value.source === "automatic"
                    ? "automatic"
                    : value.source === "manual"
                      ? "manual"
                      : "all",
                )}
                onOptionSelect={(_, d) => set("source", d.optionValue!)}
              >
                <Option value="all">{t("all")}</Option>
                <Option value="automatic">{t("automatic")}</Option>
                <Option value="manual">{t("manual")}</Option>
              </Dropdown>
            </Field>
            <Field label={t("fromDate")}>
              <Input
                type="date"
                value={value.from}
                onChange={(_, d) => set("from", d.value)}
              />
            </Field>
            <Field label={t("toDate")}>
              <Input
                type="date"
                value={value.to}
                min={value.from || undefined}
                onChange={(_, d) => set("to", d.value)}
              />
            </Field>
            {playerFilters}
            <Button
              onClick={() => {
                onChange(initialPlayFilters);
                onResetPlayer();
              }}
            >
              {t("resetFilters")}
            </Button>
          </div>
        </PopoverSurface>
      </Popover>
    </div>
  );
}
