import { describe, expect, it } from "vitest";
import { mergeWorkspace } from "./workspace-update";
import type { LiveWorkspace } from "../generated/live/LiveWorkspace";

const workspace: LiveWorkspace = {
  status: {
    state: "offline",
    message: "",
    phase: null,
    account: null,
    activeGameId: null,
    lastCheckedAt: 0,
  },
  accounts: [],
  matches: [],
  sessions: [],
  ui: {
    myAccounts: [],
    notes: [],
    reviewed: [],
    players: [],
    premades: [],
    identityLinks: [],
    theme: null,
  },
  uiRevision: 0,
  idleMinutes: 60,
};

describe("workspace synchronization", () => {
  it("updates connection state without invalidating archive and query inputs", () => {
    const status = {
      ...workspace.status,
      state: "connected",
      lastCheckedAt: 4000,
    };
    const result = mergeWorkspace(workspace, {
      cursor: 2,
      reset: false,
      removed: [],
      workspace: null,
      status,
    });
    expect(result.status).toBe(status);
    expect(result.accounts).toBe(workspace.accounts);
    expect(result.matches).toBe(workspace.matches);
    expect(result.sessions).toBe(workspace.sessions);
    expect(result.ui).toBe(workspace.ui);
  });
  it("accepts changed metadata without copying unchanged matches and requires an initial snapshot", () => {
    const ui = { ...workspace.ui, theme: "light" };
    const result = mergeWorkspace(workspace, {
      cursor: 3,
      reset: false,
      removed: [],
      workspace: { ...workspace, ui },
      status: workspace.status,
    });
    expect(result.matches).toBe(workspace.matches);
    expect(result.ui).toBe(ui);
    expect(() =>
      mergeWorkspace(undefined, {
        cursor: 1,
        reset: false,
        removed: [],
        workspace: null,
        status: workspace.status,
      }),
    ).toThrow("workspace.missingSnapshot");
  });
});
