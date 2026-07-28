import type { QueryClient } from "@tanstack/react-query";

import type {
  CollectionFolderIdInput,
  CloseRequestTabInput,
  CreateCollectionFolderInput,
  CreateSavedRequestInput,
  ExecutionRecordIdInput,
  ExecutionHistorySnapshotDto,
  MoveCollectionFolderInput,
  MoveSavedRequestInput,
  OpenSavedRequestTabInput,
  RequestDraftIdInput,
  ResolveRequestContentInput,
  RequestWorkspaceSnapshotDto,
  RenameCollectionFolderInput,
  SavedRequestIdInput,
  SelectEnvironmentInput,
  SetExecutionHistoryDisabledInput,
  SetExecutionRecordPinnedInput,
  UpdateRequestDraftInput,
  WorkspaceIdInput,
} from "./generated/ipc";
import { requestIpc } from "./ipc";

export const requestWorkspaceQueryKey = (workspaceId: string) =>
  ["requestWorkspace", workspaceId] as const;

export const executionHistoryQueryKey = (workspaceId: string) =>
  ["executionHistory", workspaceId] as const;

export const requestWorkspaceQuery = (input: WorkspaceIdInput) => ({
  queryKey: requestWorkspaceQueryKey(input.workspaceId),
  queryFn: () => requestIpc.listRequestWorkspace(input),
});

export const executionHistoryQuery = (input: WorkspaceIdInput) => ({
  queryKey: executionHistoryQueryKey(input.workspaceId),
  queryFn: () => requestIpc.listExecutionHistory(input),
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

export async function createCollectionFolder(
  queryClient: QueryClient,
  input: CreateCollectionFolderInput,
) {
  return updateRequestWorkspaceSnapshot(
    queryClient,
    input.workspaceId,
    requestIpc.createCollectionFolder(input),
  );
}

export async function selectEnvironment(
  queryClient: QueryClient,
  input: SelectEnvironmentInput,
) {
  return updateRequestWorkspaceSnapshot(
    queryClient,
    input.workspaceId,
    requestIpc.selectEnvironment(input),
  );
}

export async function resolveRequestContent(input: ResolveRequestContentInput) {
  return requestIpc.resolveRequestContent(input);
}

export async function renameCollectionFolder(
  queryClient: QueryClient,
  input: RenameCollectionFolderInput,
) {
  return updateRequestWorkspaceSnapshot(
    queryClient,
    input.workspaceId,
    requestIpc.renameCollectionFolder(input),
  );
}

export async function moveCollectionFolder(
  queryClient: QueryClient,
  input: MoveCollectionFolderInput,
) {
  return updateRequestWorkspaceSnapshot(
    queryClient,
    input.workspaceId,
    requestIpc.moveCollectionFolder(input),
  );
}

export async function duplicateCollectionFolder(
  queryClient: QueryClient,
  input: CollectionFolderIdInput,
) {
  return updateRequestWorkspaceSnapshot(
    queryClient,
    input.workspaceId,
    requestIpc.duplicateCollectionFolder(input),
  );
}

export async function deleteCollectionFolder(
  queryClient: QueryClient,
  input: CollectionFolderIdInput,
) {
  return updateRequestWorkspaceSnapshot(
    queryClient,
    input.workspaceId,
    requestIpc.deleteCollectionFolder(input),
  );
}

export async function moveSavedRequest(
  queryClient: QueryClient,
  input: MoveSavedRequestInput,
) {
  return updateRequestWorkspaceSnapshot(
    queryClient,
    input.workspaceId,
    requestIpc.moveSavedRequest(input),
  );
}

export async function duplicateSavedRequest(
  queryClient: QueryClient,
  input: SavedRequestIdInput,
) {
  return updateRequestWorkspaceSnapshot(
    queryClient,
    input.workspaceId,
    requestIpc.duplicateSavedRequest(input),
  );
}

export async function deleteSavedRequest(
  queryClient: QueryClient,
  input: SavedRequestIdInput,
) {
  return updateRequestWorkspaceSnapshot(
    queryClient,
    input.workspaceId,
    requestIpc.deleteSavedRequest(input),
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

export async function setExecutionHistoryDisabled(
  queryClient: QueryClient,
  input: SetExecutionHistoryDisabledInput,
) {
  return updateExecutionHistorySnapshot(
    queryClient,
    input.workspaceId,
    requestIpc.setExecutionHistoryDisabled(input),
  );
}

export async function setExecutionRecordPinned(
  queryClient: QueryClient,
  input: SetExecutionRecordPinnedInput,
) {
  return updateExecutionHistorySnapshot(
    queryClient,
    input.workspaceId,
    requestIpc.setExecutionRecordPinned(input),
  );
}

export async function openExecutionRecordAsDraft(
  queryClient: QueryClient,
  input: ExecutionRecordIdInput,
) {
  return updateRequestWorkspaceSnapshot(
    queryClient,
    input.workspaceId,
    requestIpc.openExecutionRecordAsDraft(input),
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

async function updateExecutionHistorySnapshot(
  queryClient: QueryClient,
  workspaceId: string,
  operation: Promise<ExecutionHistorySnapshotDto>,
) {
  const snapshot = await operation;
  queryClient.setQueryData(executionHistoryQueryKey(workspaceId), snapshot);
  return snapshot;
}
