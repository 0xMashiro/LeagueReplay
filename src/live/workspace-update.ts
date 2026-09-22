import type { LiveWorkspace } from "../generated/live/LiveWorkspace";
import type { WorkspaceUpdate } from "../generated/live/WorkspaceUpdate";

export function mergeWorkspace(
  previous: LiveWorkspace | undefined,
  update: WorkspaceUpdate,
): LiveWorkspace {
  if (!update.workspace) {
    if (!previous || update.reset) throw new Error("workspace.missingSnapshot");
    return { ...previous, status: update.status };
  }
  let matches = previous?.matches ?? [];
  if (
    update.reset ||
    update.removed.length ||
    update.workspace.matches.length
  ) {
    const records = new Map(
      (update.reset ? [] : matches).map((record) => [
        record.observationId,
        record,
      ]),
    );
    update.removed.forEach((id) => records.delete(id));
    update.workspace.matches.forEach((record) =>
      records.set(record.observationId, record),
    );
    matches = [...records.values()];
  }
  return { ...update.workspace, status: update.status, matches };
}
