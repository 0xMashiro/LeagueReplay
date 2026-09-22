import { Dropdown, Option } from "@fluentui/react-components";
import type { PlaySession } from "../generated/live/PlaySession";
import { useI18n } from "../i18n";

export function SessionSelect({
  sessions,
  value,
  onChange,
  allowNew = false,
  disabled = false,
  newLabel,
}: {
  sessions: PlaySession[];
  value: string;
  onChange: (value: string) => void;
  allowNew?: boolean;
  disabled?: boolean;
  newLabel?: string;
}) {
  const { t, date } = useI18n();
  const label = (s: PlaySession) =>
    `${s.title} · ${date(s.startedAt)} · ${t(s.endedAt === null ? "sessionActive" : "sessionArchived")}`;
  const selected = sessions.find((s) => s.id === value);
  return (
    <Dropdown
      className="session-select"
      aria-label={t("session")}
      disabled={disabled}
      selectedOptions={[value]}
      value={
        selected
          ? label(selected)
          : allowNew && value === "new"
            ? (newLabel ?? t("newSession"))
            : ""
      }
      placeholder={t("selectSession")}
      onOptionSelect={(_, data) => {
        if (data.optionValue) onChange(data.optionValue);
      }}
    >
      {allowNew && <Option value="new">{newLabel ?? t("newSession")}</Option>}
      {sessions.map((s) => (
        <Option key={s.id} value={s.id} text={label(s)}>
          {label(s)}
        </Option>
      ))}
    </Dropdown>
  );
}
