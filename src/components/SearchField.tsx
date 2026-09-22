import { useRef } from "react";
import { Button, Input, Tooltip } from "@fluentui/react-components";
import { Dismiss16Regular, Search20Regular } from "@fluentui/react-icons";
import { useI18n } from "../i18n";

export function SearchField({
  value,
  onChange,
  label,
  className,
  disabled,
}: {
  value: string;
  onChange: (value: string) => void;
  label: string;
  className?: string;
  disabled?: boolean;
}) {
  const { t } = useI18n();
  const input = useRef<HTMLInputElement>(null);
  const clear = () => {
    onChange("");
    input.current?.focus();
  };
  return (
    <Input
      ref={input}
      className={className}
      aria-label={label}
      placeholder={label}
      value={value}
      disabled={disabled}
      maxLength={200}
      contentBefore={<Search20Regular />}
      contentAfter={
        value ? (
          <Tooltip content={t("clearSearch")} relationship="label">
            <Button
              type="button"
              size="small"
              appearance="transparent"
              icon={<Dismiss16Regular />}
              aria-label={t("clearSearch")}
              disabled={disabled}
              onClick={clear}
            />
          </Tooltip>
        ) : undefined
      }
      onChange={(_, data) => onChange(data.value)}
      onKeyDown={(event) => {
        if (event.key === "Escape" && value) {
          event.stopPropagation();
          clear();
        }
      }}
    />
  );
}
