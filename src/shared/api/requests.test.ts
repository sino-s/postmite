import { QueryClient } from "@tanstack/react-query";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type {
  ExecutionHistorySnapshotDto,
  RequestWorkspaceSnapshotDto,
} from "./generated/ipc";

const requestIpcMock = vi.hoisted(() => ({
  closeRequestTab: vi.fn(),
  createCollectionFolder: vi.fn(),
  createEnvironment: vi.fn(),
  createSavedRequest: vi.fn(),
  deleteCollectionFolder: vi.fn(),
  deleteEnvironment: vi.fn(),
  deleteSavedRequest: vi.fn(),
  duplicateCollectionFolder: vi.fn(),
  duplicateSavedRequest: vi.fn(),
  generateCurl: vi.fn(),
  importCurlAsDraft: vi.fn(),
  flushRequestDrafts: vi.fn(),
  listExecutionHistory: vi.fn(),
  listRequestWorkspace: vi.fn(),
  moveCollectionFolder: vi.fn(),
  moveSavedRequest: vi.fn(),
  openExecutionRecordAsDraft: vi.fn(),
  openSavedRequestTab: vi.fn(),
  openUnsavedRequestTab: vi.fn(),
  previewCurlImport: vi.fn(),
  renameCollectionFolder: vi.fn(),
  resolveRequestContent: vi.fn(),
  saveRequestDraft: vi.fn(),
  selectEnvironment: vi.fn(),
  setExecutionHistoryDisabled: vi.fn(),
  setExecutionRecordPinned: vi.fn(),
  updateRequestDraft: vi.fn(),
  updateEnvironment: vi.fn(),
}));

vi.mock("./ipc", () => ({
  requestIpc: requestIpcMock,
}));

import {
  createEnvironment,
  deleteEnvironment,
  executionHistoryQuery,
  executionHistoryQueryKey,
  generateCurl,
  importCurlAsDraft,
  openExecutionRecordAsDraft,
  openUnsavedRequestTab,
  previewCurlImport,
  requestWorkspaceQuery,
  requestWorkspaceQueryKey,
  setExecutionHistoryDisabled,
  updateRequestDraft,
  updateEnvironment,
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
    expect(queryClient.getQueryData(requestWorkspaceQueryKey("workspace-1"))).toEqual(
      snapshot,
    );
  });

  it("keeps Environment create, update, and delete results in the scoped cache", async () => {
    const created = requestSnapshot("workspace-1");
    const updated = {
      ...requestSnapshot("workspace-1"),
      environments: [
        {
          id: "environment-1",
          workspaceId: "workspace-1",
          name: "Development",
          position: 0,
          isSelected: true,
        },
      ],
    };
    requestIpcMock.createEnvironment.mockResolvedValue(created);
    requestIpcMock.updateEnvironment.mockResolvedValue({
      snapshot: updated,
      secretPersistence: "SESSION_ONLY",
    });
    requestIpcMock.deleteEnvironment.mockResolvedValue(created);

    await createEnvironment(queryClient, {
      workspaceId: "workspace-1",
      name: "Development",
    });
    const result = await updateEnvironment(queryClient, {
      workspaceId: "workspace-1",
      environmentId: "environment-1",
      name: "Development",
      variables: [
        {
          previousName: null,
          name: "token",
          value: { type: "SECRET", value: ["typed", "ipc", "value"].join("-") },
        },
      ],
    });
    expect(result.secretPersistence).toBe("SESSION_ONLY");
    expect(queryClient.getQueryData(requestWorkspaceQueryKey("workspace-1"))).toEqual(
      updated,
    );

    await deleteEnvironment(queryClient, {
      workspaceId: "workspace-1",
      environmentId: "environment-1",
    });
    expect(queryClient.getQueryData(requestWorkspaceQueryKey("workspace-1"))).toEqual(
      created,
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

  it("previews and imports cURL through typed IPC without bypassing Rust drafts", async () => {
    const snapshot = requestSnapshot("workspace-1");
    const preview = {
      sourceName: "Pasted cURL",
      content: emptyContent(),
      warningCount: 0,
      unsupportedCount: 0,
      warnings: [],
      unsupported: [],
    };
    requestIpcMock.previewCurlImport.mockResolvedValue(preview);
    requestIpcMock.importCurlAsDraft.mockResolvedValue({
      preview,
      snapshot,
    });

    await expect(
      previewCurlImport({
        workspaceId: "workspace-1",
        sourceName: "Pasted cURL",
        command: "curl https://example.test",
      }),
    ).resolves.toBe(preview);
    const result = await importCurlAsDraft(queryClient, {
      workspaceId: "workspace-1",
      sourceName: "Pasted cURL",
      command: "curl https://example.test",
    });

    expect(result.snapshot).toBe(snapshot);
    expect(queryClient.getQueryData(requestWorkspaceQueryKey("workspace-1"))).toBe(
      snapshot,
    );
  });

  it("passes cURL generation confirmation explicitly", async () => {
    requestIpcMock.generateCurl.mockResolvedValue({
      command: "curl https://example.test --data-raw ********",
      includedSecretCount: 0,
      redactedSecretCount: 1,
    });

    await generateCurl({
      workspaceId: "workspace-1",
      environmentId: null,
      content: emptyContent(),
      resolved: null,
      includeSecrets: false,
    });

    expect(requestIpcMock.generateCurl).toHaveBeenCalledWith({
      workspaceId: "workspace-1",
      environmentId: null,
      content: emptyContent(),
      resolved: null,
      includeSecrets: false,
    });
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
        auth: { type: "NONE" },
        redirect: { enabled: true, maxRedirects: 10 },
        tls: {
          verify: true,
          customCaReference: null,
          clientCertificateReference: null,
          clientKeyReference: null,
        },
        transport: {
          proxy: {
            source: "PROCESS_ENVIRONMENT",
            url: null,
            noProxy: [],
          },
          timeouts: {
            connectMs: 10_000n,
            overallMs: 300_000n,
            idleMs: 60_000n,
          },
        },
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

function emptyContent() {
  return {
    name: "Imported cURL",
    method: "GET",
    url: "https://example.test",
    body: { type: "NONE" as const },
    query: [],
    headers: [],
    auth: { type: "NONE" as const },
    redirect: { enabled: true, maxRedirects: 10 },
    tls: {
      verify: true,
      customCaReference: null,
      clientCertificateReference: null,
      clientKeyReference: null,
    },
    transport: {
      proxy: {
        source: "PROCESS_ENVIRONMENT" as const,
        url: null,
        noProxy: [],
      },
      timeouts: {
        connectMs: 10_000n,
        overallMs: 300_000n,
        idleMs: 60_000n,
      },
    },
  };
}
