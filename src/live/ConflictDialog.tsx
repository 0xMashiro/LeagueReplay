import {
  Button,
  Dialog,
  DialogSurface,
  DialogBody,
  DialogTitle,
  DialogContent,
  DialogActions,
} from "@fluentui/react-components";
import { useI18n } from "../i18n";
import type { LiveStore } from "./useLiveWorkspace";
import { mergeUi } from "./mergeUi";
import { exportJson } from "../export-file";
export function ConflictDialog({ store }: { store: LiveStore }) {
  const { t } = useI18n();
  const value = store.conflict;
  if (!value) return null;
  const conflicts = mergeUi(value.base, value.mine, value.saved).conflicts;
  const describe = (value: unknown): string => {
    if (value === undefined) return t("entryDeleted");
    if (Array.isArray(value)) return value.map(describe).join(" · ");
    if (value && typeof value === "object") {
      const item = value as {
        body?: string;
        name?: string;
        account?: { riotId: string };
      };
      return (
        item.body ?? item.name ?? item.account?.riotId ?? t("annotationEntry")
      );
    }
    return String(value);
  };
  const exportDraft = () => {
    exportJson(value.mine, "LeagueReplay-annotations-draft.json");
  };
  return (
    <Dialog open modalType="alert">
      <DialogSurface>
        <DialogBody>
          <DialogTitle>{t("conflictTitle")}</DialogTitle>
          <DialogContent>
            <p>{t("conflictHint")}</p>
            <div className="conflict-entries">
              {conflicts.map((c, index) => (
                <div key={c.field}>
                  <strong>{t("conflictEntry", { number: index + 1 })}</strong>
                  <p>
                    {t("myVersion")}: {describe(c.mine)}
                  </p>
                  <p>
                    {t("savedVersion")}: {describe(c.saved)}
                  </p>
                </div>
              ))}
            </div>
          </DialogContent>
          <DialogActions fluid>
            <Button onClick={exportDraft}>{t("conflictExport")}</Button>
            <Button
              disabled={store.saving}
              onClick={() => void store.resolveConflict("saved")}
            >
              {t("conflictSaved")}
            </Button>
            <Button
              disabled={store.saving}
              appearance="primary"
              onClick={() => void store.resolveConflict("mine")}
            >
              {t("conflictMine")}
            </Button>
          </DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
}
