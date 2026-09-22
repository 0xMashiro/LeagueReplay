import type { CommandError } from "./generated/ipc/CommandError";
import type { Command } from "./ipc";

export class IpcError extends Error {
  readonly code: string;
  readonly command: Command;

  constructor(command: Command, failure: CommandError) {
    super(failure.code);
    this.name = "IpcError";
    this.code = failure.code;
    this.command = command;
  }

  override toString() {
    return this.code;
  }
}

export function ipcError(command: Command, payload: unknown): IpcError {
  if (
    payload !== null &&
    typeof payload === "object" &&
    Object.hasOwn(payload, "code") &&
    "code" in payload &&
    typeof payload.code === "string" &&
    payload.code.length <= 128 &&
    /^[a-z][a-zA-Z0-9]*(?:\.[a-zA-Z][a-zA-Z0-9]*)+$/.test(payload.code)
  )
    return new IpcError(command, { code: payload.code });
  // Tauri argument/transport failures are not application error envelopes.
  // Do not turn arbitrary native diagnostic text into user-facing messages.
  return new IpcError(command, { code: "ipc.transport" });
}
