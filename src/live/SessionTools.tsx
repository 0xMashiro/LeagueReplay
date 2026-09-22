import { useState } from "react";
import {
  Button,
  Dialog,
  DialogActions,
  DialogBody,
  DialogContent,
  DialogSurface,
  DialogTitle,
  Dropdown,
  Field,
  Input,
  Menu,
  MenuItem,
  MenuList,
  MenuPopover,
  MenuTrigger,
  MessageBar,
  Option,
} from "@fluentui/react-components";
import { MoreHorizontal20Regular } from "@fluentui/react-icons";
import type { PlaySession } from "../generated/live/PlaySession";
import type { LiveStore } from "./useLiveWorkspace";
import { useI18n } from "../i18n";
import { SessionSelect } from "./SessionSelect";

type Action = "renameSession" | "mergeSession" | "splitSession";

export function SessionTools({
  session,
  store,
}: {
  session: PlaySession;
  store: LiveStore;
}) {
  const { t, date, error } = useI18n();
  const [action, setAction] = useState<Action>("renameSession");
  const [opened, setOpened] = useState(false);
  const [title, setTitle] = useState("");
  const [destination, setDestination] = useState("");
  const [boundary, setBoundary] = useState("");
  const [failure, setFailure] = useState(false);
  const sessions = store.workspace!.sessions;
  const choices = sessions.filter((s) => s.id !== session.id);
  const target = choices.find((s) => s.id === destination);
  const index = session.segments.findIndex((s) => s.id === boundary);
  const count = (s: PlaySession) =>
    s.segments.reduce((n, segment) => n + segment.matchIds.length, 0);
  const moving =
    index > 0
      ? session.segments.slice(index).reduce((n, s) => n + s.matchIds.length, 0)
      : 0;
  const nameValid =
    title.trim().length > 0 &&
    [...title.trim()].length <= 80 &&
    ![...title].some(
      (character) =>
        character.charCodeAt(0) < 32 || character.charCodeAt(0) === 127,
    );
  const valid =
    action === "mergeSession"
      ? !!target
      : nameValid && (action !== "splitSession" || index > 0);
  const open = (value: Action) => {
    setAction(value);
    setOpened(true);
    setFailure(false);
    setTitle(value === "splitSession" ? t("splitDefault") : session.title);
    setDestination(choices[0]?.id ?? "");
    setBoundary(session.segments[1]?.id ?? "");
  };
  const submit = async () => {
    if (!valid) return;
    const ok = await store.command(
      action === "mergeSession"
        ? [
            "merge_live_sessions",
            { sourceId: session.id, destinationId: destination },
          ]
        : action === "splitSession"
          ? [
              "split_live_session",
              {
                sessionId: session.id,
                segmentId: boundary,
                title: title.trim(),
              },
            ]
          : [
              "rename_live_session",
              { sessionId: session.id, title: title.trim() },
            ],
    );
    if (ok) {
      setOpened(false);
      // Merging removes this component; return keyboard focus to the destination.
      if (action === "mergeSession")
        requestAnimationFrame(() => focusMenu(destination));
    } else setFailure(true);
  };
  return (
    <>
      <Menu>
        <MenuTrigger disableButtonEnhancement>
          <Button
            appearance="subtle"
            icon={<MoreHorizontal20Regular />}
            disabled={store.busy}
            aria-label={t("organizeSession", { title: session.title })}
            data-session-menu={session.id}
          />
        </MenuTrigger>
        <MenuPopover>
          <MenuList>
            <MenuItem onClick={() => open("renameSession")}>
              {t("renameSession")}
            </MenuItem>
            <MenuItem
              disabled={!choices.length}
              onClick={() => open("mergeSession")}
            >
              {t("mergeSession")}
            </MenuItem>
            <MenuItem
              disabled={session.segments.length < 2}
              onClick={() => open("splitSession")}
            >
              {t("splitSession")}
            </MenuItem>
          </MenuList>
        </MenuPopover>
      </Menu>
      <Dialog
        open={opened}
        surfaceMotion={{
          onMotionFinish: (_, data) => {
            if (data.direction === "exit") focusMenu(session.id);
          },
        }}
        onOpenChange={(_, data) => {
          if (!data.open && !store.busy) setOpened(false);
        }}
      >
        <DialogSurface className="session-dialog">
          <DialogBody>
            <DialogTitle>{t(action)}</DialogTitle>
            <DialogContent>
              <form
                id={`session-${session.id}`}
                className="live-form"
                onSubmit={(event) => {
                  event.preventDefault();
                  if (!store.busy) void submit();
                }}
              >
                {failure && store.error && (
                  <MessageBar intent="error">{error(store.error)}</MessageBar>
                )}
                <p className="session-context">
                  {session.title} · {date(session.startedAt)} ·{" "}
                  {t("sessionGames", { count: count(session) })}
                </p>
                {action === "mergeSession" ? (
                  <>
                    <Field label={t("session")}>
                      <SessionSelect
                        sessions={choices}
                        value={destination}
                        onChange={setDestination}
                        disabled={store.busy}
                      />
                    </Field>
                    {target && (
                      <p className="session-operation-summary">
                        {t("mergeSummary", {
                          title: target.title,
                          count: count(session) + count(target),
                        })}
                      </p>
                    )}
                    {(session.endedAt === null || target?.endedAt === null) && (
                      <p className="muted">{t("mergeRecording")}</p>
                    )}
                  </>
                ) : (
                  <>
                    {action === "splitSession" && (
                      <>
                        <Field label={t("splitBoundary")}>
                          <Dropdown
                            className="session-select"
                            aria-label={t("splitBoundary")}
                            disabled={store.busy}
                            selectedOptions={[boundary]}
                            value={
                              index > 0
                                ? t("segmentLabel", {
                                    number: index + 1,
                                    name:
                                      store.workspace!.accounts.find(
                                        (a) =>
                                          a.id ===
                                          session.segments[index].accountId,
                                      )?.riotId ?? "",
                                    count:
                                      session.segments[index].matchIds.length,
                                  })
                                : ""
                            }
                            onOptionSelect={(_, data) => {
                              if (data.optionValue)
                                setBoundary(data.optionValue);
                            }}
                          >
                            {session.segments.slice(1).map((segment, i) => {
                              const label = t("segmentLabel", {
                                number: i + 2,
                                name:
                                  store.workspace!.accounts.find(
                                    (a) => a.id === segment.accountId,
                                  )?.riotId ?? "",
                                count: segment.matchIds.length,
                              });
                              return (
                                <Option key={segment.id} value={segment.id}>
                                  {label}
                                </Option>
                              );
                            })}
                          </Dropdown>
                        </Field>
                        <p className="session-operation-summary">
                          {t("splitSummary", {
                            moving,
                            remaining: count(session) - moving,
                          })}
                        </p>
                        {session.endedAt === null && (
                          <p className="muted">{t("splitRecording")}</p>
                        )}
                      </>
                    )}
                    <Field
                      label={t("sessionName")}
                      validationState={nameValid ? "none" : "error"}
                      validationMessage={
                        !nameValid ? t("sessionTitleError") : undefined
                      }
                    >
                      <Input
                        value={title}
                        maxLength={80}
                        disabled={store.busy}
                        onChange={(_, data) => setTitle(data.value)}
                      />
                    </Field>
                  </>
                )}
                {action !== "renameSession" && (
                  <p className="muted">{t("sessionKeepFacts")}</p>
                )}
              </form>
            </DialogContent>
            <DialogActions>
              <Button disabled={store.busy} onClick={() => setOpened(false)}>
                {t("cancel")}
              </Button>
              <Button
                appearance="primary"
                type="submit"
                form={`session-${session.id}`}
                disabled={store.busy || !valid}
              >
                {t(store.busy ? "sessionSaving" : "sessionConfirm")}
              </Button>
            </DialogActions>
          </DialogBody>
        </DialogSurface>
      </Dialog>
    </>
  );
}

function focusMenu(id: string) {
  document
    .querySelector<HTMLButtonElement>(`[data-session-menu="${CSS.escape(id)}"]`)
    ?.focus();
}
