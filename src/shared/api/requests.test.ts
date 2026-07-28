import { QueryClient } from "@tanstack/react-query";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type {
  ExecutionHistorySnapshotDto,
  RequestWorkspaceSnapshotDto,
} from "./generated/ipc";

const requestIpcMock = vi.hoisted(() => ({
  closeRequestTab: vi.fn(),
  createCollectionFolder: vi.fn(),
  createSavedRequest: vi.fn(),
  deleteCollectionFolder: vi.fn(),
  deleteSavedRequest: vi.fn(),
  duplicateCollectionFolder: vi.fn(),
  duplicateSavedRequest: vi.fn(),
  flushRequestDrafts: vi.fn(),
  listExecutionHistory: vi.fn(),
  listRequestWorkspace: vi.fn(),
  moveCollectionFolder: vi.fn(),
  moveSavedRequest: vi.fn(),
  openExecutionRecordAsDraft: vi.fn(),
  openSavedRequestTab: vi.fn(),
  openUnsavedRequestTab: vi.fn(),
  renameCollectionFolder: vi.fn(),
  resolveRequestContent: vi.fn(),
  saveRequestDraft: vi.fn(),
  selectEnvironment: vi.fn(),
  setExecutionHistoryDisabled: vi.fn(),
  setExecutionRecordPinned: vi.fn(),
  updateRequestDraft: vi.fn(),
}));

vi.mock("./ipc", () => ({
  requestIpc: requestIpcMock,
}));

import {
  executionHistoryQuery,
  executionHistoryQueryKey,
  openExecutionRecordAsDraft,
  openUnsavedRequestTab,
  requestWorkspaceQuery,
  requestWorkspaceQueryKey,
  setExecutionHistoryDisabled,
  updateRequestDraft,
} from "./requests";

describe("request query API", () => {
  let queryClient: QueryClient;

  beforeEach(() => {
    queryClient = new QueryClient();
    vi.clearAllMocks();
  });

  it("uses one stable key per persisted request workspace snapshot", async () => {
    const snapshot = requestSnapshot("workspace-1");
    requestIpcMock.listRequestWorkspace.mockResolvedValue(snapshot);

    await expect(
      requestWorkspaceQuery({ workspaceId: "workspace-1" }).queryFn(),
    ).resolves.toBe(snapshot);

    expect(requestWorkspaceQuery({ workspaceId: "workspace-1" }).queryKey).toEqual(
      requestWorkspaceQueryKey("workspace-1"),
    );
    expect(requestIpcMock.listRequestWorkspace).toHaveBeenCalledWith({
      workspaceId: "workspace-1",
    });
  });

  it("writes snapshot-returning mutations into the request workspace cache", async () => {
    const snapshot = requestSnapshot("workspace-1");
    requestIpcMock.openUnsavedRequestTab.mockResolvedValue(snapshot);

    const result = await openUnsavedRequestTab(queryClient, {
      workspaceId: "workspace-1",
    });

    expect(result).toBe(snapshot);
    expect(queryClient.getQueryData(requestWorkspaceQueryKey("workspace-1"))).toBe(
      snapshot,
    );
  });

  it("uses a stable key and cache updates for execution history", async () => {
    const history = executionHistorySnapshot("workspace-1");
    requestIpcMock.listExecutionHistory.mockResolvedValue(history);
    requestIpcMock.setExecutionHistoryDisabled.mockResolvedValue({
      ...history,
      disabled: true,
    });

    await expect(
      executionHistoryQuery({ workspaceId: "workspace-1" }).queryFn(),
    ).resolves.toBe(history);
    const disabled = await setExecutionHistoryDisabled(queryClient, {
      workspaceId: "workspace-1",
      disabled: true,
    });

    expect(executionHistoryQuery({ workspaceId: "workspace-1" }).queryKey).toEqual(
      executionHistoryQueryKey("workspace-1"),
    );
    expect(disabled.disabled).toBe(true);
    expect(queryClient.getQueryData(executionHistoryQueryKey("workspace-1"))).toBe(
      disabled,
    );
  });

  it("opens an execution record as a draft into the request workspace cache", async () => {
    const snapshot = requestSnapshot("workspace-1");
    requestIpcMock.openExecutionRecordAsDraft.mockResolvedValue(snapshot);

    const result = await openExecutionRecordAsDraft(queryClient, {
      workspaceId: "workspace-1",
      recordId: "history-1",
    });

    expect(result).toBe(snapshot);
    expect(queryClient.getQueryData(requestWorkspaceQueryKey("workspace-1"))).toBe(
      snapshot,
    );
  });

  it("queues draft updates without mutating the cached saved request snapshot", async () => {
    const existing = requestSnapshot("workspace-1");
    queryClient.setQueryData(requestWorkspaceQueryKey("workspace-1"), existing);
    requestIpcMock.updateRequestDraft.mockResolvedValue(undefined);

    await updateRequestDraft({
      workspaceId: "workspace-1",
      draftId: "draft-1",
      content: {
        name: "Edited draft",
        method: "GET",
        url: "https://example.test",
        body: { type: "NONE" },
        query: [],
        headers: [],
      },
    });

    expect(requestIpcMock.updateRequestDraft).toHaveBeenCalledOnce();
    expect(queryClient.getQueryData(requestWorkspaceQueryKey("workspace-1"))).toBe(
      existing,
    );
  });
});

function requestSnapshot(workspaceId: string): RequestWorkspaceSnapshotDto {
  return {
    workspaceId,
    collectionFolders: [],
    environments: [],
    collectionVariables: [],
    environmentVariables: [],
    savedRequests: [],
    drafts: [],
    tabs: [],
  };
}

function executionHistorySnapshot(workspaceId: string): ExecutionHistorySnapshotDto {
  return {
    workspaceId,
    disabled: false,
    records: [],
    warning:
      "Unknown sensitive values inside arbitrary response bodies may not always be detected.",
  };
}
