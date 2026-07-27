import type { QueryClient } from "@tanstack/react-query";

import type {
  CloseRequestTabInput,
  CreateSavedRequestInput,
  OpenSavedRequestTabInput,
  RequestDraftIdInput,
  RequestWorkspaceSnapshotDto,
  UpdateRequestDraftInput,
  WorkspaceIdInput,
} from "./generated/ipc";
import { requestIpc } from "./ipc";

export const requestWorkspaceQueryKey = (workspaceId: string) =>
  ["requestWorkspace", workspaceId] as const;

export const requestWorkspaceQuery = (input: WorkspaceIdInput) => ({
  queryKey: requestWorkspaceQueryKey(input.workspaceId),
  queryFn: () => requestIpc.listRequestWorkspace(input),
});

export async function openUnsavedRequestTab(
  queryClient: QueryClient,
  input: WorkspaceIdInput,
) {
  return updateRequestWorkspaceSnapshot(
    queryClient,
    input.workspaceId,
    requestIpc.openUnsavedRequestTab(input),
  );
}

export async function createSavedRequest(
  queryClient: QueryClient,
  input: CreateSavedRequestInput,
) {
  return updateRequestWorkspaceSnapshot(
    queryClient,
    input.workspaceId,
    requestIpc.createSavedRequest(input),
  );
}

export async function openSavedRequestTab(
  queryClient: QueryClient,
  input: OpenSavedRequestTabInput,
) {
  return updateRequestWorkspaceSnapshot(
    queryClient,
    input.workspaceId,
    requestIpc.openSavedRequestTab(input),
  );
}

export async function updateRequestDraft(input: UpdateRequestDraftInput) {
  await requestIpc.updateRequestDraft(input);
}

export async function flushRequestDrafts() {
  await requestIpc.flushRequestDrafts();
}

export async function saveRequestDraft(
  queryClient: QueryClient,
  input: RequestDraftIdInput,
) {
  return updateRequestWorkspaceSnapshot(
    queryClient,
    input.workspaceId,
    requestIpc.saveRequestDraft(input),
  );
}

export async function closeRequestTab(
  queryClient: QueryClient,
  input: CloseRequestTabInput,
) {
  return updateRequestWorkspaceSnapshot(
    queryClient,
    input.workspaceId,
    requestIpc.closeRequestTab(input),
  );
}

async function updateRequestWorkspaceSnapshot(
  queryClient: QueryClient,
  workspaceId: string,
  operation: Promise<RequestWorkspaceSnapshotDto>,
) {
  const snapshot = await operation;
  queryClient.setQueryData(requestWorkspaceQueryKey(workspaceId), snapshot);
  return snapshot;
}
