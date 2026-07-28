import type { QueryClient } from "@tanstack/react-query";

import type {
  CreateWorkspaceInput,
  RenameWorkspaceInput,
  SetWorkspaceBaseDirectoryInput,
  WorkspaceIdInput,
  WorkspaceSnapshotDto,
} from "./generated/ipc";
import { workspaceIpc } from "./ipc";

export const workspaceQueryKey = ["workspaces"] as const;

export const workspaceQuery = {
  queryKey: workspaceQueryKey,
  queryFn: () => workspaceIpc.listWorkspaces(),
};

export async function createWorkspace(
  queryClient: QueryClient,
  input: CreateWorkspaceInput,
) {
  return updateWorkspaceSnapshot(
    queryClient,
    workspaceIpc.createWorkspace(input),
  );
}

export async function renameWorkspace(
  queryClient: QueryClient,
  input: RenameWorkspaceInput,
) {
  return updateWorkspaceSnapshot(
    queryClient,
    workspaceIpc.renameWorkspace(input),
  );
}

export async function setWorkspaceBaseDirectory(
  queryClient: QueryClient,
  input: SetWorkspaceBaseDirectoryInput,
) {
  return updateWorkspaceSnapshot(
    queryClient,
    workspaceIpc.setWorkspaceBaseDirectory(input),
  );
}

export async function switchWorkspace(
  queryClient: QueryClient,
  input: WorkspaceIdInput,
) {
  return updateWorkspaceSnapshot(
    queryClient,
    workspaceIpc.switchWorkspace(input),
  );
}

export async function deleteWorkspace(
  queryClient: QueryClient,
  input: WorkspaceIdInput,
) {
  return updateWorkspaceSnapshot(
    queryClient,
    workspaceIpc.deleteWorkspace(input),
  );
}

async function updateWorkspaceSnapshot(
  queryClient: QueryClient,
  operation: Promise<WorkspaceSnapshotDto>,
) {
  const snapshot = await operation;
  queryClient.setQueryData(workspaceQueryKey, snapshot);
  return snapshot;
}
