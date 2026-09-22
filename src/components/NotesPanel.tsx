import { ui, useI18n } from "../i18n";
import { useEffect, useRef, useState, type RefObject } from "react";
import {
  Badge,
  Button,
  Dialog,
  DialogActions,
  DialogBody,
  DialogContent,
  DialogSurface,
  DialogTitle,
  Field,
  Input,
  Textarea,
  MessageBar,
  MessageBarBody,
  MessageBarActions,
  useRestoreFocusTarget,
} from "@fluentui/react-components";
import {
  Add20Regular,
  Delete20Regular,
  Edit20Regular,
  Note24Regular,
} from "@fluentui/react-icons";
import { useArchive } from "../archive-context";
import { formatTime, parseTime } from "../domain/archive";
import type { Match, Note } from "../domain/types";

export interface NoteContext {
  at?: number;
  participantId?: number;
  label: string;
  nonce: number;
}

export interface NoteDraft {
  body: string;
  time: string;
  tags: string;
  participantId?: number;
  editing?: string;
  contextNonce?: number;
}

export function NotesPanel({
  match,
  context,
  draft,
  onSeek,
  onDirtyChange,
}: {
  match: Pick<Match, "id" | "duration" | "participants">;
  context?: NoteContext;
  draft?: RefObject<NoteDraft | undefined>;
  onSeek?: (at: number) => void;
  onDirtyChange: (dirty: boolean) => void;
}) {
  const { t, error: explain } = useI18n();
  const restoreFocusTarget = useRestoreFocusTarget();
  const { state, update, saving, error: storageError, retry } = useArchive();
  const [body, setBody] = useState(draft?.current?.body ?? "");
  const [time, setTime] = useState(draft?.current?.time ?? "");
  const [tags, setTags] = useState(draft?.current?.tags ?? "");
  const [participantId, setParticipantId] = useState<number | undefined>(
    draft?.current?.participantId,
  );
  const [editing, setEditing] = useState<string | undefined>(
    draft?.current?.editing,
  );
  const [deleting, setDeleting] = useState<string>();
  const [error, setError] = useState("");
  const [saved, setSaved] = useState(false);
  const editor = useRef<HTMLTextAreaElement>(null);
  const contextNonce = useRef(draft?.current?.contextNonce);
  useEffect(() => {
    if (draft)
      draft.current = {
        body,
        time,
        tags,
        participantId,
        editing,
        contextNonce: contextNonce.current,
      };
  }, [draft, body, time, tags, participantId, editing]);
  useEffect(() => {
    if (context && contextNonce.current !== context.nonce) {
      contextNonce.current = context.nonce;
      setTime(context.at === undefined ? "" : formatTime(context.at));
      setParticipantId(context.participantId);
      requestAnimationFrame(() => editor.current?.focus());
    }
  }, [context]);
  useEffect(() => {
    onDirtyChange(Boolean(body.trim() || tags.trim() || editing));
  }, [body, tags, editing, onDirtyChange]);
  const notes = state.notes
    .filter((note) => note.matchId === match.id)
    .toSorted((a, b) => (a.at ?? -1) - (b.at ?? -1));
  const clear = () => {
    setBody("");
    setTime("");
    setTags("");
    setEditing(undefined);
    setParticipantId(undefined);
    setError("");
  };
  const save = () => {
    try {
      const at = parseTime(time, match.duration);
      if (!body.trim()) throw new Error(ui("写下一点想留住的内容"));
      const note: Note = {
        id: editing ?? crypto.randomUUID(),
        matchId: match.id,
        body: body.trim(),
        at,
        participantId,
        tags: [
          ...new Set(
            tags
              .split(/[,，、]/)
              .map((tag) => tag.trim())
              .filter(Boolean),
          ),
        ].slice(0, 5),
        updatedAt: new Date().toISOString(),
      };
      update((current) => ({
        ...current,
        notes: [
          ...current.notes.filter((existing) => existing.id !== note.id),
          note,
        ],
      }));
      clear();
      setSaved(true);
    } catch (reason) {
      setError((reason as Error).message);
    }
  };
  const edit = (note: Note) => {
    setEditing(note.id);
    setBody(note.body);
    setTime(note.at === undefined ? "" : formatTime(note.at));
    setTags(note.tags.join("，"));
    setParticipantId(note.participantId);
    setSaved(false);
  };
  return (
    <div className="notes-panel">
      {storageError && (
        <MessageBar intent="error">
          <MessageBarBody>{explain(storageError)}</MessageBarBody>
          <MessageBarActions>
            <Button onClick={retry}>{ui("重试保存")}</Button>
          </MessageBarActions>
        </MessageBar>
      )}
      <div className="notes-heading">
        <Note24Regular />
        <h2>{ui("复盘笔记")}</h2>
        <Badge appearance="tint" color="subtle">
          {notes.length}
        </Badge>
      </div>
      <div className="note-list">
        {notes.map((note) => (
          <article key={note.id} className="saved-note">
            <div className="saved-note-meta">
              {note.at !== undefined && onSeek ? (
                <Button
                  size="small"
                  appearance="subtle"
                  title={t("seekNote")}
                  onClick={() => onSeek(note.at!)}
                >
                  {formatTime(note.at)} ↗
                </Button>
              ) : (
                <span>
                  {note.at === undefined ? ui("整局") : formatTime(note.at)}
                </span>
              )}
              <small>
                {note.participantId
                  ? match.participants.find(
                      (person) => person.id === note.participantId,
                    )?.name
                  : ui("对局笔记")}
              </small>
            </div>
            <p>{note.body}</p>
            <div className="note-tags">
              {note.tags.map((tag) => (
                <Badge
                  key={tag}
                  appearance="outline"
                  color="subtle"
                  size="small"
                >
                  {tag}
                </Badge>
              ))}
            </div>
            <div className="note-actions">
              <Button
                size="small"
                appearance="subtle"
                icon={<Edit20Regular />}
                aria-label={ui("编辑笔记")}
                disabled={!!body && editing !== note.id}
                onClick={() => edit(note)}
              />
              <Button
                size="small"
                appearance="subtle"
                icon={<Delete20Regular />}
                aria-label={ui("删除笔记")}
                onClick={() => setDeleting(note.id)}
              />
            </div>
          </article>
        ))}
        {!notes.length && (
          <div className="notes-empty">
            {ui("这一局，还没有笔记。")}
            <br />
            {ui("从一个值得回看的时刻开始。")}
          </div>
        )}
      </div>
      <form
        className="note-editor"
        onSubmit={(event) => {
          event.preventDefault();
          save();
        }}
        onKeyDown={(event) => {
          if ((event.ctrlKey || event.metaKey) && event.key === "Enter") {
            event.preventDefault();
            save();
          }
        }}
      >
        <h3>{editing ? ui("编辑笔记") : ui("写一条笔记")}</h3>
        {context && !editing && participantId !== undefined && (
          <div className="note-context">{context.label}</div>
        )}
        <Field label={ui("内容")}>
          <Textarea
            ref={editor}
            textarea={{ ...restoreFocusTarget, as: "textarea" }}
            aria-label={ui("笔记内容")}
            placeholder={ui("这波为什么打赢了？下次可以怎么做…")}
            resize="vertical"
            value={body}
            maxLength={5000}
            onChange={(_, data) => {
              setBody(data.value);
              setSaved(false);
            }}
          />
        </Field>
        <div className="note-fields">
          <Field
            label={ui("时间点")}
            validationMessage={error}
            validationState={error ? "error" : "none"}
          >
            <Input
              aria-label={ui("笔记时间点")}
              placeholder={ui("整局 / 18:42")}
              value={time}
              onChange={(_, data) => {
                setTime(data.value);
                setError("");
              }}
            />
          </Field>
          <Field label={ui("标签")}>
            <Input
              aria-label={ui("笔记标签")}
              placeholder={ui("团战，出装")}
              value={tags}
              maxLength={100}
              onChange={(_, data) => setTags(data.value)}
            />
          </Field>
        </div>
        <div className="note-save-row">
          <span aria-live="polite" className="muted">
            {storageError
              ? ui("尚未保存，请重试")
              : saving
                ? ui("正在保存…")
                : saved
                  ? ui("已保存到本地归档")
                  : ui("Ctrl + Enter 保存")}
          </span>
          {editing && (
            <Button size="small" onClick={clear}>
              {ui("取消")}
            </Button>
          )}
          <Button
            type="submit"
            size="small"
            appearance="primary"
            icon={<Add20Regular />}
            disabled={!body.trim()}
          >
            {ui("保存笔记")}
          </Button>
        </div>
      </form>
      <Dialog
        open={!!deleting}
        onOpenChange={(_, data) => {
          if (!data.open) setDeleting(undefined);
        }}
      >
        <DialogSurface>
          <DialogBody>
            <DialogTitle>{ui("删除这条笔记？")}</DialogTitle>
            <DialogContent>
              {ui("删除后无法恢复，对局数据会保留。")}
            </DialogContent>
            <DialogActions>
              <Button onClick={() => setDeleting(undefined)}>
                {ui("取消")}
              </Button>
              <Button
                appearance="primary"
                onClick={() => {
                  update((current) => ({
                    ...current,
                    notes: current.notes.filter((note) => note.id !== deleting),
                  }));
                  if (editing === deleting) clear();
                  setDeleting(undefined);
                }}
              >
                {ui("删除")}
              </Button>
            </DialogActions>
          </DialogBody>
        </DialogSurface>
      </Dialog>
    </div>
  );
}
