import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "./ipc";
import { IpcError } from "./ipc-error";
import { explainError, translate } from "./i18n";

const nativeInvoke = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke: nativeInvoke }));
beforeEach(() => {
  nativeInvoke.mockReset();
});

describe("IPC failure boundary", () => {
  it("preserves the conflict code and originating command for recovery", async () => {
    nativeInvoke.mockRejectedValue({ code: "ui.conflict" });
    const failure = await invoke("live_workspace").catch(
      (reason: unknown) => reason,
    );
    expect(failure).toBeInstanceOf(IpcError);
    expect(failure).toMatchObject({
      code: "ui.conflict",
      command: "live_workspace",
    });
    expect(explainError(failure)).toBe(translate("conflictTitle"));
  });

  it("does not accept legacy strings or expose malformed transport diagnostics", async () => {
    for (const payload of [
      "backup.changed",
      "private native path",
      null,
      { code: 4 },
      { code: "数据库内部错误" },
      Object.create({ code: "ui.conflict" }),
    ]) {
      nativeInvoke.mockRejectedValue(payload);
      const failure = await invoke("choose_backup").catch(
        (reason: unknown) => reason,
      );
      expect(failure).toMatchObject({
        code: "ipc.transport",
        command: "choose_backup",
      });
      expect(explainError(failure)).toBe(translate("generalError"));
    }
  });
});
