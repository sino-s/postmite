import { QueryClient } from "@tanstack/react-query";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { WorkspaceSnapshotDto } from "./generated/ipc";

const workspaceIpcMock = vi.hoisted(() => ({
  createWorkspace: vi.fn(),
  deleteWorkspace: vi.fn(),
  listWorkspaces: vi.fn(),
  renameWorkspace: vi.fn(),
  switchWorkspace: vi.fn(),
}));

vi.mock("./ipc", () => ({
  workspaceIpc: workspaceIpcMock,
}));

import {
  createWorkspace,
  renameWorkspace,
  workspaceQuery,
  workspaceQueryKey,
} from "./workspaces";

describe("workspace query API", () => {
  let queryClient: QueryClient;

  beforeEach(() => {
    queryClient = new QueryClient();
    vi.clearAllMocks();
  });

  it("uses one stable key for persisted workspace snapshots", async () => {
    const snapshot = workspaceSnapshot("workspace-1", "Personal");
    workspaceIpcMock.listWorkspaces.mockResolvedValue(snapshot);

    await expect(workspaceQuery.queryFn()).resolves.toBe(snapshot);

    expect(workspaceQuery.queryKey).toBe(workspaceQueryKey);
    expect(workspaceIpcMock.listWorkspaces).toHaveBeenCalledWith();
  });

  it("writes mutation results into the workspace snapshot cache", async () => {
    const snapshot = workspaceSnapshot("workspace-2", "Client");
    workspaceIpcMock.createWorkspace.mockResolvedValue(snapshot);

    const result = await createWorkspace(queryClient, { name: "Client" });

    expect(result).toBe(snapshot);
    expect(queryClient.getQueryData(workspaceQueryKey)).toBe(snapshot);
  });

  it("does not update the cache when a mutation fails", async () => {
    const existing = workspaceSnapshot("workspace-1", "Personal");
    queryClient.setQueryData(workspaceQueryKey, existing);
    workspaceIpcMock.renameWorkspace.mockRejectedValue(
      new Error("sentinel mutation failure"),
    );

    await expect(
      renameWorkspace(queryClient, {
        workspaceId: "workspace-1",
        name: "Renamed",
      }),
    ).rejects.toThrow("sentinel mutation failure");

    expect(queryClient.getQueryData(workspaceQueryKey)).toBe(existing);
  });
});

function workspaceSnapshot(id: string, name: string): WorkspaceSnapshotDto {
  return {
    selectedWorkspaceId: id,
    workspaces: [{ id, name, isSelected: true, baseDirectory: null }],
  };
}
