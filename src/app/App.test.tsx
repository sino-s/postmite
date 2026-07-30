import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { axe } from "jest-axe";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type {
  CookieJarSnapshotDto,
  ExecutionEventDto,
  ExecutionEventKindDto,
  ExecutionHistorySnapshotDto,
  ExecutionRecordDto,
  RequestContentDto,
  RequestWorkspaceSnapshotDto,
  WorkspaceSnapshotDto,
} from "../shared/api/generated/ipc";
import {
  cookieJarQueryKey,
  executionHistoryQueryKey,
  requestWorkspaceQueryKey,
} from "../shared/api/requests";
import { workspaceQueryKey } from "../shared/api/workspaces";
import { App } from "./App";
import { queryFromUrl } from "../features/request-editor/models/ordered-fields";

const workspaceApiMock = vi.hoisted(() => ({
  workspaceQuery: {
    queryKey: ["workspaces"] as const,
    queryFn: vi.fn(),
  },
  workspaceQueryKey: ["workspaces"] as const,
}));

const requestApiMock = vi.hoisted(() => ({
  closeRequestTab: vi.fn(),
  clearCookies: vi.fn(),
  cookieJarQuery: vi.fn(),
  cookieJarQueryKey: (workspaceId: string) => ["cookieJar", workspaceId] as const,
  createCollectionFolder: vi.fn(),
  deleteCookie: vi.fn(),
  deleteCollectionFolder: vi.fn(),
  deleteSavedRequest: vi.fn(),
  duplicateCollectionFolder: vi.fn(),
  duplicateSavedRequest: vi.fn(),
  executionHistoryQuery: vi.fn(),
  executionHistoryQueryKey: (workspaceId: string) =>
    ["executionHistory", workspaceId] as const,
  generateCurl: vi.fn(),
  moveCollectionFolder: vi.fn(),
  moveSavedRequest: vi.fn(),
  openExecutionRecordAsDraft: vi.fn(),
  openSavedRequestTab: vi.fn(),
  openUnsavedRequestTab: vi.fn(),
  requestWorkspaceQuery: vi.fn(),
  requestWorkspaceQueryKey: (workspaceId: string) =>
    ["requestWorkspace", workspaceId] as const,
  renameCollectionFolder: vi.fn(),
  revealCookieValue: vi.fn(),
  resolveRequestContent: vi.fn(),
  saveRequestDraft: vi.fn(),
  selectEnvironment: vi.fn(),
  setExecutionHistoryDisabled: vi.fn(),
  setExecutionRecordPinned: vi.fn(),
  upsertCookie: vi.fn(),
  updateRequestDraft: vi.fn(),
}));

const executionApiMock = vi.hoisted(() => ({
  cancelRequestExecution: vi.fn(),
  emitExecutionEvent: vi.fn(),
  listenToRequestExecutionEvents: vi.fn(),
  recordFrontendExecutionTrace: vi.fn(),
  startRequestExecution: vi.fn(),
}));

const clipboardApiMock = vi.hoisted(() => ({
  writeClipboardText: vi.fn(),
}));

vi.mock("../shared/api/workspaces", () => workspaceApiMock);
vi.mock("../shared/api/requests", () => requestApiMock);
vi.mock("../shared/api/clipboard", () => clipboardApiMock);
vi.mock("../shared/api/execution", async (importActual) => {
  const actual =
    await importActual<typeof import("../shared/api/execution")>();
  return {
    ...actual,
    cancelRequestExecution: executionApiMock.cancelRequestExecution,
    listenToRequestExecutionEvents:
      executionApiMock.listenToRequestExecutionEvents,
    recordFrontendExecutionTrace:
      executionApiMock.recordFrontendExecutionTrace,
    startRequestExecution: executionApiMock.startRequestExecution,
  };
});
vi.mock("../features/request-editor/editors/CodeMirrorBodyEditor", () => ({
  CodeMirrorBodyEditor: ({
    value,
    onChange,
  }: {
    value: string;
    onChange: (value: string) => void;
  }) => (
    <textarea
      aria-label="Raw body editor"
      onChange={(event) => onChange(event.currentTarget.value)}
      value={value}
    />
  ),
}));

describe("App request editor", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    window.localStorage.clear();
    vi.spyOn(globalThis.crypto, "randomUUID").mockReturnValue(
      "execution-1" as ReturnType<Crypto["randomUUID"]>,
    );
    workspaceApiMock.workspaceQuery.queryFn.mockResolvedValue(
      workspaceSnapshot(),
    );
    requestApiMock.requestWorkspaceQuery.mockImplementation(
      ({ workspaceId }: { workspaceId: string }) => ({
        queryKey: requestWorkspaceQueryKey(workspaceId),
        queryFn: vi.fn().mockResolvedValue(emptyRequestSnapshot()),
      }),
    );
    requestApiMock.executionHistoryQuery.mockImplementation(
      ({ workspaceId }: { workspaceId: string }) => ({
        queryKey: executionHistoryQueryKey(workspaceId),
        queryFn: vi.fn().mockResolvedValue(emptyExecutionHistorySnapshot()),
      }),
    );
    requestApiMock.cookieJarQuery.mockImplementation(
      ({ workspaceId }: { workspaceId: string }) => ({
        queryKey: cookieJarQueryKey(workspaceId),
        queryFn: vi.fn().mockResolvedValue(emptyCookieJarSnapshot()),
      }),
    );
    requestApiMock.resolveRequestContent.mockResolvedValue(emptyResolution());
    requestApiMock.generateCurl.mockResolvedValue({
      command: "curl 'https://example.test'",
      includedSecretCount: 0,
      redactedSecretCount: 0,
    });
    clipboardApiMock.writeClipboardText.mockResolvedValue(undefined);
    requestApiMock.revealCookieValue.mockResolvedValue({ value: "sid-value" });
    executionApiMock.startRequestExecution.mockImplementation(
      async (input: { executionId: string }) => ({
        status: "queued",
        executionId: input.executionId,
        initialEvents: [],
      }),
    );
    executionApiMock.cancelRequestExecution.mockResolvedValue({
      executionId: "execution-1",
      cancelled: true,
    });
    executionApiMock.listenToRequestExecutionEvents.mockImplementation(
      async (onEvent: (event: ExecutionEventDto) => void) => {
        executionApiMock.emitExecutionEvent.mockImplementation(onEvent);
        return vi.fn();
      },
    );
  });

  it("creates, edits, saves, and closes a request tab", async () => {
    const user = userEvent.setup();
    const queryClient = renderApp(emptyRequestSnapshot());
    const opened = requestSnapshot({
      content: requestContent({ url: "https://example.test" }),
      isDirty: true,
    });
    const saved = requestSnapshot({
      content: requestContent({
        url: "https://example.test/users?tag=first&tag=",
      }),
      isDirty: false,
      savedRequestId: "saved-1",
    });

    requestApiMock.openUnsavedRequestTab.mockImplementation(
      async (client: QueryClient) => {
        client.setQueryData(requestWorkspaceQueryKey("workspace-1"), opened);
        return opened;
      },
    );
    requestApiMock.updateRequestDraft.mockResolvedValue(undefined);
    requestApiMock.saveRequestDraft.mockImplementation(
      async (client: QueryClient) => {
        client.setQueryData(requestWorkspaceQueryKey("workspace-1"), saved);
        return saved;
      },
    );
    requestApiMock.closeRequestTab.mockImplementation(
      async (client: QueryClient) => {
        const empty = emptyRequestSnapshot();
        client.setQueryData(requestWorkspaceQueryKey("workspace-1"), empty);
        return empty;
      },
    );

    await user.click(await screen.findByRole("button", { name: "New Request" }));
    replaceInputText("Name", "Create user");
    await user.selectOptions(screen.getByLabelText("Method"), "POST");
    replaceInputText("URL", "https://example.test/users?tag=first&tag=");
    await user.click(screen.getByRole("tab", { name: "Body" }));
    await user.click(await screen.findByRole("button", { name: "Raw" }));
    replaceInputText("Raw body editor", "{\"ok\":true}");
    await user.click(screen.getByRole("button", { name: "Save" }));
    await user.click(screen.getByRole("button", { name: "Close Untitled Request" }));

    await waitFor(() =>
      expect(requestApiMock.saveRequestDraft).toHaveBeenCalledWith(
        queryClient,
        {
          workspaceId: "workspace-1",
          draftId: "draft-1",
        },
      ),
    );
    expect(requestApiMock.updateRequestDraft).toHaveBeenLastCalledWith({
      workspaceId: "workspace-1",
      draftId: "draft-1",
      content: expect.objectContaining({
        method: "POST",
        name: "Create user",
        body: { type: "RAW", content: "{\"ok\":true}" },
        query: [
          { enabled: true, order: 0, name: "tag", value: "first" },
          { enabled: true, order: 1, name: "tag", value: "" },
        ],
      }),
    });
    expect(requestApiMock.closeRequestTab).toHaveBeenCalledWith(queryClient, {
      workspaceId: "workspace-1",
      tabId: "tab-1",
      decision: "DISCARD",
    });
  });

  it("copies the latest unsaved Draft through typed cURL generation with ordered fields", async () => {
    const user = userEvent.setup();
    const content = requestContent({
      method: "POST",
      url: "https://example.test/items?tag=first&tag=second",
      query: [
        { enabled: true, order: 0, name: "tag", value: "first" },
        { enabled: false, order: 1, name: "tag", value: "off" },
        { enabled: true, order: 2, name: "tag", value: "second" },
      ],
      headers: [
        { enabled: true, order: 0, name: "X-Mode", value: "first" },
        { enabled: false, order: 1, name: "X-Mode", value: "off" },
        { enabled: true, order: 2, name: "X-Mode", value: "second" },
      ],
      auth: { type: "BASIC", username: "public-user", password: "" },
      body: { type: "RAW", content: "{\"draft\":true}" },
    });
    const resolved = {
      ...emptyResolution(),
      url: {
        value: "https://example.test/items?tag=first&tag=second",
        containsSecret: false,
      },
      query: content.query.map((field) => ({
        enabled: field.enabled,
        order: field.order,
        name: { value: field.name, containsSecret: false },
        value: { value: field.value, containsSecret: false },
      })),
      headers: content.headers.map((field) => ({
        enabled: field.enabled,
        order: field.order,
        name: { value: field.name, containsSecret: false },
        value: { value: field.value, containsSecret: false },
      })),
      body: { value: "{\"draft\":true}", containsSecret: false },
    };
    requestApiMock.resolveRequestContent.mockResolvedValue(resolved);
    requestApiMock.generateCurl.mockResolvedValue({
      command: "curl 'https://example.test/items?tag=first&tag=second'",
      includedSecretCount: 0,
      redactedSecretCount: 0,
    });
    renderApp(requestSnapshot({ content, isDirty: true }));

    await user.selectOptions(screen.getByLabelText("Method"), "PATCH");
    const copyButton = await screen.findByRole("button", { name: "Copy cURL" });
    await waitFor(() => expect(copyButton).toBeEnabled());
    copyButton.focus();
    await user.keyboard("{Enter}");

    await waitFor(() =>
      expect(requestApiMock.generateCurl).toHaveBeenCalledWith({
        workspaceId: "workspace-1",
        environmentId: null,
        content: expect.objectContaining({
          method: "PATCH",
          url: "https://example.test/items?tag=first&tag=second",
          query: content.query,
          headers: content.headers,
          auth: content.auth,
          body: content.body,
        }),
        resolved,
        includeSecrets: false,
      }),
    );
    expect(clipboardApiMock.writeClipboardText).toHaveBeenCalledWith(
      "curl 'https://example.test/items?tag=first&tag=second'",
    );
    expect(await screen.findByText("cURL copied.")).toBeInTheDocument();
  });

  it("defaults Secret confirmation to redacted copy and requires explicit inclusion", async () => {
    const user = userEvent.setup();
    requestApiMock.generateCurl
      .mockResolvedValueOnce({
        command: "curl 'https://redacted.example.test'",
        includedSecretCount: 0,
        redactedSecretCount: 1,
      })
      .mockResolvedValueOnce({
        command: "curl 'https://redacted.example.test'",
        includedSecretCount: 0,
        redactedSecretCount: 1,
      })
      .mockResolvedValueOnce({
        command: "curl 'https://redacted.example.test'",
        includedSecretCount: 0,
        redactedSecretCount: 1,
      })
      .mockResolvedValueOnce({
        command: "curl 'https://included.example.test'",
        includedSecretCount: 1,
        redactedSecretCount: 0,
      });
    renderApp(requestSnapshot({ isDirty: true }));
    const copyButton = await screen.findByRole("button", { name: "Copy cURL" });
    await waitFor(() => expect(copyButton).toBeEnabled());

    await user.click(copyButton);
    expect(
      await screen.findByRole("button", { name: "Copy redacted cURL" }),
    ).toHaveFocus();
    await user.click(screen.getByRole("button", { name: "Cancel" }));
    expect(clipboardApiMock.writeClipboardText).not.toHaveBeenCalled();

    await user.click(copyButton);
    await user.click(
      await screen.findByRole("button", { name: "Copy redacted cURL" }),
    );
    expect(clipboardApiMock.writeClipboardText).toHaveBeenLastCalledWith(
      "curl 'https://redacted.example.test'",
    );

    await user.click(copyButton);
    await user.click(
      await screen.findByRole("button", { name: "Include Secrets and copy" }),
    );
    await waitFor(() =>
      expect(requestApiMock.generateCurl).toHaveBeenLastCalledWith({
        workspaceId: "workspace-1",
        environmentId: null,
        content: expect.any(Object),
        resolved: expect.any(Object),
        includeSecrets: true,
      }),
    );
    expect(clipboardApiMock.writeClipboardText).toHaveBeenLastCalledWith(
      "curl 'https://included.example.test'",
    );
  });

  it("rejects generated output when the Draft changes during generation", async () => {
    const user = userEvent.setup();
    const generated = deferred<{
      command: string;
      includedSecretCount: number;
      redactedSecretCount: number;
    }>();
    requestApiMock.generateCurl.mockReturnValue(generated.promise);
    renderApp(requestSnapshot({ isDirty: true }));
    const copyButton = await screen.findByRole("button", { name: "Copy cURL" });
    await waitFor(() => expect(copyButton).toBeEnabled());

    await user.click(copyButton);
    replaceInputText("URL", "https://changed.example.test");
    generated.resolve({
      command: "curl 'https://old.example.test'",
      includedSecretCount: 0,
      redactedSecretCount: 0,
    });

    expect(
      await screen.findByText(
        "The request changed before cURL could be copied. Try again.",
      ),
    ).toBeInTheDocument();
    expect(clipboardApiMock.writeClipboardText).not.toHaveBeenCalled();
  });

  it("invalidates Secret confirmation when the selected Environment changes", async () => {
    const user = userEvent.setup();
    const queryClient = renderApp(
      environmentRequestSnapshot({ selectedEnvironmentId: "env-dev" }),
    );
    const switched = environmentRequestSnapshot({
      selectedEnvironmentId: "env-prod",
    });
    requestApiMock.selectEnvironment.mockImplementation(
      async (client: QueryClient) => {
        client.setQueryData(requestWorkspaceQueryKey("workspace-1"), switched);
        return switched;
      },
    );
    requestApiMock.generateCurl.mockResolvedValue({
      command: "curl 'https://redacted.example.test'",
      includedSecretCount: 0,
      redactedSecretCount: 1,
    });
    const copyButton = await screen.findByRole("button", { name: "Copy cURL" });
    await waitFor(() => expect(copyButton).toBeEnabled());
    await user.click(copyButton);
    expect(
      await screen.findByRole("alertdialog", {
        name: "This cURL contains Secret values",
      }),
    ).toBeVisible();
    expect(requestApiMock.generateCurl).toHaveBeenLastCalledWith(
      expect.objectContaining({
        workspaceId: "workspace-1",
        environmentId: "env-dev",
        includeSecrets: false,
      }),
    );

    await user.selectOptions(screen.getByLabelText("Environment"), "env-prod");
    expect(requestApiMock.selectEnvironment).toHaveBeenCalledWith(queryClient, {
      workspaceId: "workspace-1",
      environmentId: "env-prod",
    });
    await waitFor(() =>
      expect(
        screen.queryByRole("alertdialog", {
          name: "This cURL contains Secret values",
        }),
      ).not.toBeInTheDocument(),
    );
    expect(clipboardApiMock.writeClipboardText).not.toHaveBeenCalled();
  });

  it("disables cURL copy while resolution is in flight", async () => {
    const resolution = deferred<ReturnType<typeof emptyResolution>>();
    requestApiMock.resolveRequestContent.mockReturnValue(resolution.promise);
    renderApp(requestSnapshot({ isDirty: true }));
    const copyButton = await screen.findByRole("button", { name: "Copy cURL" });
    expect(copyButton).toBeDisabled();

    resolution.resolve(emptyResolution());
    await waitFor(() => expect(copyButton).toBeEnabled());
  });

  it("reports clipboard failure without exposing generated output", async () => {
    const user = userEvent.setup();
    requestApiMock.generateCurl.mockResolvedValue({
      command: "curl 'https://private-output.example.test'",
      includedSecretCount: 0,
      redactedSecretCount: 0,
    });
    clipboardApiMock.writeClipboardText.mockRejectedValue(
      new Error("clipboard unavailable"),
    );
    renderApp(requestSnapshot({ isDirty: true }));
    const copyButton = await screen.findByRole("button", { name: "Copy cURL" });
    await waitFor(() => expect(copyButton).toBeEnabled());
    await user.click(copyButton);

    const alert = await screen.findByRole("alert");
    expect(alert).toHaveTextContent("cURL could not be copied. Try again.");
    expect(alert).not.toHaveTextContent("private-output");
  });

  it("keeps URL query text and Params rows bidirectionally synchronized", async () => {
    renderApp(
      requestSnapshot({
        content: requestContent({
          url: "https://example.test/search?tag=first&tag=&empty=",
        }),
        isDirty: true,
      }),
    );

    expect(await screen.findByLabelText("Params row 1 name")).toHaveValue("tag");
    expect(screen.getByLabelText("Params row 2 name")).toHaveValue("tag");
    expect(screen.getByLabelText("Params row 2 value")).toHaveValue("");
    expect(screen.getByLabelText("Params row 3 name")).toHaveValue("empty");

    replaceInputText("Params row 2 value", "second");

    expect(screen.getByLabelText("URL")).toHaveValue(
      "https://example.test/search?tag=first&tag=second&empty=",
    );
  });

  it("runs save and request execution keyboard shortcuts", async () => {
    const user = userEvent.setup();
    const queryClient = renderApp(
      requestSnapshot({ content: requestContent(), isDirty: true }),
    );
    requestApiMock.updateRequestDraft.mockResolvedValue(undefined);
    requestApiMock.saveRequestDraft.mockImplementation(
      async (client: QueryClient) => {
        const saved = requestSnapshot({
          content: requestContent(),
          isDirty: false,
        });
        client.setQueryData(requestWorkspaceQueryKey("workspace-1"), saved);
        return saved;
      },
    );

    await screen.findByRole("button", { name: "Send" });
    screen.getByLabelText("URL").focus();
    await user.keyboard("{Control>}s{/Control}");
    await user.keyboard("{Control>}{Enter}{/Control}");

    await waitFor(() =>
      expect(requestApiMock.saveRequestDraft).toHaveBeenCalledWith(
        queryClient,
        {
          workspaceId: "workspace-1",
          draftId: "draft-1",
        },
      ),
    );
    expect(executionApiMock.startRequestExecution).toHaveBeenCalledWith({
      workspaceId: "workspace-1",
      draftId: "draft-1",
      executionId: "execution-1",
      content: requestContent(),
    });
    expect(screen.getByRole("button", { name: "Cancel" })).toBeEnabled();
  });

  it("disables Send and exposes listener registration failures", async () => {
    executionApiMock.listenToRequestExecutionEvents.mockRejectedValueOnce(
      new Error("event.listen denied"),
    );
    renderApp(requestSnapshot({ content: requestContent(), isDirty: true }));

    expect(
      await screen.findByText(
        "Response event listener is unavailable. Restart Postmite and try again.",
      ),
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Send" })).toBeDisabled();
    expect(executionApiMock.startRequestExecution).not.toHaveBeenCalled();
  });

  it("displays status headers JSON body and timing from execution events", async () => {
    const user = userEvent.setup();
    renderApp(
      requestSnapshot({ content: requestContent(), isDirty: true }),
    );
    requestApiMock.updateRequestDraft.mockResolvedValue(undefined);

    await user.click(await screen.findByRole("button", { name: "Send" }));
    executionApiMock.emitExecutionEvent(
      executionEvent("execution-1", 1n, {
        type: "STARTED",
        method: "GET",
        url: "https://example.test",
        tlsVerification: true,
      }),
    );
    executionApiMock.emitExecutionEvent(
      executionEvent("execution-1", 2n, {
        type: "RESPONSE_HEADERS",
        status: 200,
        headers: [{ name: "content-type", value: "application/json" }],
      }),
    );
    executionApiMock.emitExecutionEvent(
      executionEvent("execution-1", 3n, {
        type: "COMPLETED",
        status: 200,
        bodyPreview: "{\"ok\":true}",
        bodyTruncated: false,
      }),
    );

    expect(await screen.findByText("Completed")).toBeInTheDocument();
    expect(screen.getByText("Status 200")).toBeInTheDocument();
    expect(screen.getByText(/"ok": true/)).toBeInTheDocument();
    await user.click(
      within(screen.getByRole("tablist", { name: "Response details" })).getByRole(
        "tab",
        { name: "Headers" },
      ),
    );
    expect(screen.getByText("content-type")).toBeInTheDocument();
    expect(screen.getByText("application/json")).toBeInTheDocument();
    expect(screen.getByText(/^Time \d+ ms$/)).toBeInTheDocument();
    expect(screen.getByText(/Timing queue 0 ms/)).toBeInTheDocument();
    expect(executionApiMock.recordFrontendExecutionTrace).toHaveBeenCalledWith(
      "execution-1",
      "EVENT_APPLIED",
      3n,
    );
  });

  it("applies fast response events that arrive before start returns", async () => {
    const user = userEvent.setup();
    renderApp(
      requestSnapshot({ content: requestContent(), isDirty: true }),
    );
    requestApiMock.updateRequestDraft.mockResolvedValue(undefined);
    executionApiMock.startRequestExecution.mockImplementationOnce(async (input) => {
      executionApiMock.emitExecutionEvent(
        executionEvent(input.executionId, 1n, {
          type: "COMPLETED",
          status: 200,
          bodyPreview: "fast response",
          bodyTruncated: false,
        }),
      );
      return { status: "queued", executionId: input.executionId, initialEvents: [] };
    });

    await user.click(await screen.findByRole("button", { name: "Send" }));

    expect(await screen.findByText("Completed")).toBeInTheDocument();
    expect(screen.getByText("fast response")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Cancel" })).toBeDisabled();
    expect(screen.queryByText("Running")).not.toBeInTheDocument();
  });

  it("applies terminal events returned with the start result", async () => {
    const user = userEvent.setup();
    renderApp(
      requestSnapshot({ content: requestContent(), isDirty: true }),
    );
    requestApiMock.updateRequestDraft.mockResolvedValue(undefined);
    executionApiMock.startRequestExecution.mockResolvedValueOnce({
      status: "queued",
      executionId: "execution-initial",
      initialEvents: [
        executionEvent("execution-initial", 1n, {
          type: "COMPLETED",
          status: 200,
          bodyPreview: "returned response",
          bodyTruncated: false,
        }),
      ],
    });

    await user.click(await screen.findByRole("button", { name: "Send" }));

    expect(await screen.findByText("Completed")).toBeInTheDocument();
    expect(screen.getByText("returned response")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Send" })).toBeEnabled();
    expect(executionApiMock.recordFrontendExecutionTrace).toHaveBeenCalledWith(
      "execution-initial",
      "START_RECONCILED_TERMINAL",
    );
  });

  it("classifies a buffered terminal event from start reconciliation as terminal", async () => {
    const user = userEvent.setup();
    renderApp(
      requestSnapshot({ content: requestContent(), isDirty: true }),
    );
    requestApiMock.updateRequestDraft.mockResolvedValue(undefined);
    executionApiMock.startRequestExecution.mockImplementationOnce(async () => {
      executionApiMock.emitExecutionEvent(
        executionEvent("execution-buffered", 1n, {
          type: "COMPLETED",
          status: 200,
          bodyPreview: "buffered response",
          bodyTruncated: false,
        }),
      );
      return {
        status: "queued",
        executionId: "execution-buffered",
        initialEvents: [],
      };
    });

    await user.click(await screen.findByRole("button", { name: "Send" }));

    expect(await screen.findByText("Completed")).toBeInTheDocument();
    expect(screen.getByText("buffered response")).toBeInTheDocument();
    expect(executionApiMock.recordFrontendExecutionTrace).toHaveBeenCalledWith(
      "execution-buffered",
      "EVENT_BUFFERED",
      1n,
    );
    expect(executionApiMock.recordFrontendExecutionTrace).toHaveBeenCalledWith(
      "execution-buffered",
      "START_RECONCILED_TERMINAL",
    );
  });

  it("marks pre-registered executions failed when start rejects", async () => {
    const user = userEvent.setup();
    renderApp(
      requestSnapshot({ content: requestContent(), isDirty: true }),
    );
    requestApiMock.updateRequestDraft.mockResolvedValue(undefined);
    executionApiMock.startRequestExecution.mockRejectedValueOnce(
      new Error("request input is invalid"),
    );

    await user.click(await screen.findByRole("button", { name: "Send" }));

    expect(await screen.findByText("Failed")).toBeInTheDocument();
    expect(screen.getByText("request input is invalid")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Send" })).toBeEnabled();
    expect(screen.getByRole("button", { name: "Cancel" })).toBeDisabled();
  });

  it("cancels an in-flight execution through typed IPC", async () => {
    const user = userEvent.setup();
    renderApp(
      requestSnapshot({ content: requestContent(), isDirty: true }),
    );
    requestApiMock.updateRequestDraft.mockResolvedValue(undefined);

    await user.click(await screen.findByRole("button", { name: "Send" }));
    await user.click(screen.getByRole("button", { name: "Cancel" }));
    executionApiMock.emitExecutionEvent(
      executionEvent("execution-1", 1n, { type: "CANCELLED" }),
    );

    expect(executionApiMock.cancelRequestExecution).toHaveBeenCalledWith({
      executionId: "execution-1",
    });
    expect(await screen.findByText("Cancelled")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Send" })).toBeEnabled();
  });

  it("asks before closing a tab with a running execution and cancels it", async () => {
    const user = userEvent.setup();
    const confirm = vi.spyOn(window, "confirm").mockReturnValue(true);
    const queryClient = renderApp(
      requestSnapshot({ content: requestContent(), isDirty: true }),
    );
    const empty = emptyRequestSnapshot();
    requestApiMock.updateRequestDraft.mockResolvedValue(undefined);
    requestApiMock.closeRequestTab.mockImplementation(
      async (client: QueryClient) => {
        client.setQueryData(requestWorkspaceQueryKey("workspace-1"), empty);
        return empty;
      },
    );

    await user.click(await screen.findByRole("button", { name: "Send" }));
    await user.click(screen.getByRole("button", { name: "Close Untitled Request" }));

    expect(confirm).toHaveBeenCalledWith(
      "This request is still running. Cancel it and close the tab?",
    );
    expect(executionApiMock.cancelRequestExecution).toHaveBeenCalledWith({
      executionId: "execution-1",
    });
    expect(requestApiMock.closeRequestTab).toHaveBeenCalledWith(queryClient, {
      workspaceId: "workspace-1",
      tabId: "tab-1",
      decision: "SAVE",
    });
    confirm.mockRestore();
  });

  it("keeps execution responses isolated by tab", async () => {
    const user = userEvent.setup();
    renderApp(twoTabRequestSnapshot());
    requestApiMock.updateRequestDraft.mockResolvedValue(undefined);
    executionApiMock.startRequestExecution
      .mockResolvedValueOnce({
        status: "queued",
        executionId: "execution-1",
        initialEvents: [],
      })
      .mockResolvedValueOnce({
        status: "queued",
        executionId: "execution-2",
        initialEvents: [],
      });

    await user.click(await screen.findByRole("button", { name: "Send" }));
    executionApiMock.emitExecutionEvent(
      executionEvent("execution-1", 1n, {
        type: "COMPLETED",
        status: 200,
        bodyPreview: "first tab",
        bodyTruncated: false,
      }),
    );
    await user.click(screen.getByRole("button", { name: "Second Request" }));
    expect(screen.queryByText("first tab")).not.toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Send" }));
    executionApiMock.emitExecutionEvent(
      executionEvent("execution-2", 1n, {
        type: "COMPLETED",
        status: 201,
        bodyPreview: "second tab",
        bodyTruncated: false,
      }),
    );

    expect(await screen.findByText("second tab")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Untitled Request" }));
    expect(screen.getByText("first tab")).toBeInTheDocument();
    expect(screen.queryByText("second tab")).not.toBeInTheDocument();
  });

  it("restores existing tabs from the request workspace snapshot", async () => {
    renderApp(
      requestSnapshot({
        content: requestContent({ name: "Restored Request" }),
        tabTitle: "Restored Request",
      }),
    );

    expect(
      await screen.findByRole("button", { name: "Restored Request" }),
    ).toHaveAttribute("aria-current", "page");
    expect(screen.getByLabelText("URL")).toHaveValue("https://example.test");
  });

  it("runs collection tree pointer actions through typed request APIs", async () => {
    const user = userEvent.setup();
    const promptSpy = vi.spyOn(window, "prompt").mockReturnValue("Renamed");
    const snapshot = collectionRequestSnapshot({ includeMoveTarget: true });
    renderApp(snapshot);
    requestApiMock.createCollectionFolder.mockResolvedValue(snapshot);
    requestApiMock.renameCollectionFolder.mockResolvedValue(snapshot);
    requestApiMock.moveSavedRequest.mockResolvedValue(snapshot);
    requestApiMock.duplicateSavedRequest.mockResolvedValue(snapshot);
    requestApiMock.deleteSavedRequest.mockResolvedValue(snapshot);

    await user.click(await screen.findByRole("button", { name: "New root folder" }));
    await user.click(screen.getAllByRole("button", { name: "Rename folder" })[0]);
    const dataTransfer = {
      data: "",
      effectAllowed: "move",
      getData: vi.fn(() => dataTransfer.data),
      setData: vi.fn((_type: string, value: string) => {
        dataTransfer.data = value;
      }),
    };
    fireEvent.dragStart(screen.getByRole("treeitem", { name: "Saved Request" }).parentElement!, {
      dataTransfer,
    });
    fireEvent.drop(screen.getByRole("treeitem", { name: "Archive" }).parentElement!, {
      dataTransfer,
    });
    const reorderTransfer = {
      data: "",
      effectAllowed: "move",
      getData: vi.fn(() => reorderTransfer.data),
      setData: vi.fn((_type: string, value: string) => {
        reorderTransfer.data = value;
      }),
    };
    fireEvent.dragStart(screen.getByRole("treeitem", { name: "Later Request" }).parentElement!, {
      dataTransfer: reorderTransfer,
    });
    fireEvent.drop(screen.getByRole("treeitem", { name: "Saved Request" }).parentElement!, {
      dataTransfer: reorderTransfer,
    });
    await user.click(screen.getAllByRole("button", { name: "Duplicate request" })[0]);
    await user.click(screen.getAllByRole("button", { name: "Delete request" })[0]);

    expect(requestApiMock.createCollectionFolder).toHaveBeenCalledWith(
      expect.any(QueryClient),
      {
        workspaceId: "workspace-1",
        parentCollectionId: null,
        name: "Renamed",
      },
    );
    expect(requestApiMock.renameCollectionFolder).toHaveBeenCalledWith(
      expect.any(QueryClient),
      {
        workspaceId: "workspace-1",
        collectionId: "collection-1",
        name: "Renamed",
      },
    );
    expect(requestApiMock.moveSavedRequest).toHaveBeenCalledWith(
      expect.any(QueryClient),
      {
        workspaceId: "workspace-1",
        savedRequestId: "saved-1",
        location: { collectionId: "collection-2", position: 0 },
      },
    );
    expect(requestApiMock.moveSavedRequest).toHaveBeenCalledWith(
      expect.any(QueryClient),
      {
        workspaceId: "workspace-1",
        savedRequestId: "saved-2",
        location: { collectionId: "collection-1", position: 0 },
      },
    );
    expect(requestApiMock.duplicateSavedRequest).toHaveBeenCalledWith(
      expect.any(QueryClient),
      { workspaceId: "workspace-1", savedRequestId: "saved-1" },
    );
    expect(requestApiMock.deleteSavedRequest).toHaveBeenCalledWith(
      expect.any(QueryClient),
      { workspaceId: "workspace-1", savedRequestId: "saved-1" },
    );
    promptSpy.mockRestore();
  });

  it("switches environments and shows resolved masked variable values", async () => {
    const user = userEvent.setup();
    const queryClient = renderApp(environmentRequestSnapshot());
    const switched = environmentRequestSnapshot({ selectedEnvironmentId: "env-prod" });
    requestApiMock.selectEnvironment.mockImplementation(
      async (client: QueryClient) => {
        client.setQueryData(requestWorkspaceQueryKey("workspace-1"), switched);
        return switched;
      },
    );
    requestApiMock.resolveRequestContent.mockResolvedValue({
      url: { value: "https://prod.example.test/users", containsSecret: false },
      body: { value: "", containsSecret: false },
      query: [],
      headers: [],
      references: [
        {
          name: "baseUrl",
          source: "ENVIRONMENT",
          value: {
            value: "https://prod.example.test",
            containsSecret: false,
          },
        },
        {
          name: "token",
          source: "ENVIRONMENT",
          value: { value: "********", containsSecret: true },
        },
      ],
      errors: [],
    });

    await screen.findByLabelText("Environment");
    await user.selectOptions(screen.getByLabelText("Environment"), "env-prod");

    expect(requestApiMock.selectEnvironment).toHaveBeenCalledWith(queryClient, {
      workspaceId: "workspace-1",
      environmentId: "env-prod",
    });
    expect(requestApiMock.updateRequestDraft).not.toHaveBeenCalled();
    await user.click(screen.getByRole("tab", { name: "Variables" }));
    expect(await screen.findByText("baseUrl")).toBeInTheDocument();
    expect(screen.getAllByText("Environment").length).toBeGreaterThan(1);
    expect(screen.getByText("https://prod.example.test")).toBeInTheDocument();
    expect(screen.getByText("********")).toBeInTheDocument();
  });

  it("opens a saved request from the tree with keyboard and focuses the existing tab", async () => {
    const user = userEvent.setup();
    const queryClient = renderApp(collectionRequestSnapshot());
    const focused = collectionRequestSnapshot({ activeSavedRequestId: "saved-1" });
    requestApiMock.openSavedRequestTab.mockImplementation(
      async (client: QueryClient) => {
        client.setQueryData(requestWorkspaceQueryKey("workspace-1"), focused);
        return focused;
      },
    );

    const treeItem = await screen.findByRole("treeitem", { name: "Saved Request" });
    treeItem.focus();
    await user.keyboard("{Enter}");

    expect(requestApiMock.openSavedRequestTab).toHaveBeenCalledWith(queryClient, {
      workspaceId: "workspace-1",
      savedRequestId: "saved-1",
    });
    expect(screen.getByRole("button", { name: "Saved Request" })).toHaveAttribute(
      "aria-current",
      "page",
    );
  });

  it("groups request options behind tabs and toggles the response split", async () => {
    const user = userEvent.setup();
    renderApp(
      requestSnapshot({
        content: requestContent({
          headers: [{ enabled: true, order: 0, name: "Accept", value: "application/json" }],
        }),
        isDirty: true,
      }),
      executionHistorySnapshot(),
      cookieJarSnapshot(),
    );

    expect(await screen.findByRole("tablist", { name: "Request option tabs" })).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: "Params" })).toHaveAttribute("aria-selected", "true");
    expect(screen.getByRole("region", { name: "Response" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Pin history record" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Inspect sid cookie" })).not.toBeInTheDocument();

    await user.click(screen.getByRole("tab", { name: "Headers" }));
    const headersTab = screen.getByRole("tab", { name: "Headers" });
    expect(headersTab).toHaveAttribute("aria-selected", "true");
    expect(headersTab).toHaveFocus();
    expect(screen.getByLabelText("Headers row 1 name")).toBeInTheDocument();

    await user.keyboard("{ArrowRight}");
    await waitFor(() =>
      expect(screen.getByRole("tab", { name: "Body" })).toHaveAttribute("aria-selected", "true"),
    );
    expect(screen.getByRole("button", { name: "Raw" })).toBeInTheDocument();

    await user.keyboard("{Home}");
    await waitFor(() =>
      expect(screen.getByRole("tab", { name: "Params" })).toHaveAttribute("aria-selected", "true"),
    );
    await user.keyboard("{End}");
    await waitFor(() =>
      expect(screen.getByRole("tab", { name: "Cookies" })).toHaveAttribute("aria-selected", "true"),
    );
    expect(screen.getByRole("button", { name: "Inspect sid cookie" })).toBeInTheDocument();

    expect(screen.getByRole("button", { name: "Stack request options above response" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    await user.click(screen.getByRole("button", { name: "Place request options beside response" }));
    expect(screen.getByRole("button", { name: "Place request options beside response" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    expect(window.localStorage.getItem("postmite.requestResponseSplit")).toBe("vertical");
  });

  it("pins disables and opens execution history without mutating saved requests", async () => {
    const user = userEvent.setup();
    const queryClient = renderApp(
      requestSnapshot({ content: requestContent(), isDirty: true }),
      executionHistorySnapshot(),
    );
    const opened = requestSnapshot({
      content: requestContent({
        name: "History Request",
        url: "https://history.example.test",
      }),
      isDirty: true,
      tabTitle: "History Request",
    });
    requestApiMock.setExecutionRecordPinned.mockResolvedValue(
      executionHistorySnapshot({ pinned: true }),
    );
    requestApiMock.setExecutionHistoryDisabled.mockResolvedValue(
      executionHistorySnapshot({ disabled: true }),
    );
    requestApiMock.openExecutionRecordAsDraft.mockImplementation(
      async (client: QueryClient) => {
        client.setQueryData(requestWorkspaceQueryKey("workspace-1"), opened);
        return opened;
      },
    );

    await user.click(await screen.findByRole("tab", { name: "History" }));
    await user.click(await screen.findByRole("button", { name: "Pin history record" }));
    await user.click(screen.getByLabelText("Disable history"));
    await user.click(screen.getByRole("button", { name: /History Request/ }));

    expect(requestApiMock.setExecutionRecordPinned).toHaveBeenCalledWith(
      queryClient,
      { workspaceId: "workspace-1", recordId: "history-1", pinned: true },
    );
    expect(requestApiMock.setExecutionHistoryDisabled).toHaveBeenCalledWith(
      queryClient,
      { workspaceId: "workspace-1", disabled: true },
    );
    expect(requestApiMock.openExecutionRecordAsDraft).toHaveBeenCalledWith(
      queryClient,
      { workspaceId: "workspace-1", recordId: "history-1" },
    );
    expect(screen.getByRole("button", { name: /^History Request \*$/ })).toHaveAttribute(
      "aria-current",
      "page",
    );
  });

  it("inspects edits deletes and clears cookies without exposing values by default", async () => {
    const user = userEvent.setup();
    const confirmSpy = vi.spyOn(window, "confirm").mockReturnValue(true);
    const queryClient = renderApp(
      requestSnapshot({ content: requestContent(), isDirty: true }),
      emptyExecutionHistorySnapshot(),
      cookieJarSnapshot(),
    );

    await user.click(await screen.findByRole("tab", { name: "Cookies" }));
    expect(await screen.findByText("Value ********")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Inspect sid cookie" }));
    expect(await screen.findByText("Value sid-value")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Edit sid cookie" }));
    replaceInputText("Cookie value", "updated-value");
    await user.click(screen.getByRole("button", { name: "Update cookie" }));
    await user.click(screen.getByRole("button", { name: "Delete sid cookie" }));
    await user.click(screen.getByRole("button", { name: "Clear cookies" }));

    expect(requestApiMock.revealCookieValue).toHaveBeenCalledWith({
      workspaceId: "workspace-1",
      cookieId: "cookie-1",
    });
    expect(requestApiMock.upsertCookie).toHaveBeenCalledWith(queryClient, {
      workspaceId: "workspace-1",
      cookieId: "cookie-1",
      name: "sid",
      value: "updated-value",
      domain: "example.test",
      path: "/",
      secure: true,
      httpOnly: true,
      sameSite: "LAX",
      expiresAtEpochSeconds: 1_900_000_000n,
    });
    expect(requestApiMock.deleteCookie).toHaveBeenCalledWith(queryClient, {
      workspaceId: "workspace-1",
      cookieId: "cookie-1",
    });
    expect(requestApiMock.clearCookies).toHaveBeenCalledWith(queryClient, {
      workspaceId: "workspace-1",
    });
    expect(confirmSpy).toHaveBeenCalledWith(
      "Reveal the sid cookie value? This may expose a Secret on screen.",
    );
    confirmSpy.mockRestore();
  });

  it("requires confirmation before revealing cookie values", async () => {
    const user = userEvent.setup();
    const confirmSpy = vi.spyOn(window, "confirm").mockReturnValue(false);
    renderApp(
      requestSnapshot({ content: requestContent(), isDirty: true }),
      emptyExecutionHistorySnapshot(),
      cookieJarSnapshot(),
    );

    await user.click(await screen.findByRole("tab", { name: "Cookies" }));
    expect(await screen.findByText("Value ********")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Inspect sid cookie" }));

    expect(requestApiMock.revealCookieValue).not.toHaveBeenCalled();
    expect(screen.queryByText("Value sid-value")).not.toBeInTheDocument();
    confirmSpy.mockRestore();
  });

  it("has no automated accessibility violations in the editor shell", async () => {
    renderApp(
      requestSnapshot({ content: requestContent(), isDirty: true }),
    );

    await screen.findByLabelText("Request editor");
    const results = await axe(document.body);

    expect(results.violations).toEqual([]);
  });

  it("persists accessible theme and density choices and announces request execution", async () => {
    const user = userEvent.setup();
    renderApp(requestSnapshot({ content: requestContent(), isDirty: true }));

    await user.click(screen.getByRole("button", { name: "Application menu" }));
    await screen.findByLabelText("Theme");
    await user.selectOptions(screen.getByLabelText("Theme"), "dark");
    await user.selectOptions(screen.getByLabelText("Density"), "compact");

    expect(document.documentElement).toHaveAttribute("data-theme", "dark");
    expect(document.documentElement).toHaveAttribute("data-resolved-theme", "dark");
    expect(document.documentElement).toHaveAttribute("data-density", "compact");
    expect(getComputedStyle(document.documentElement).getPropertyValue("--content-gap").trim()).toBe("0.625rem");
    expect(screen.getByRole("status")).toHaveTextContent("Ready to send request.");

    await user.click(screen.getByRole("button", { name: "Send" }));
    expect(screen.getByRole("status")).toHaveTextContent("Request is running.");
  });
});

function renderApp(
  snapshot: RequestWorkspaceSnapshotDto,
  history: ExecutionHistorySnapshotDto = emptyExecutionHistorySnapshot(),
  cookies: CookieJarSnapshotDto = emptyCookieJarSnapshot(),
) {
  const queryClient = new QueryClient({
    defaultOptions: {
      queries: { retry: false },
      mutations: { retry: false },
    },
  });
  queryClient.setQueryData(workspaceQueryKey, workspaceSnapshot());
  queryClient.setQueryData(requestWorkspaceQueryKey("workspace-1"), snapshot);
  queryClient.setQueryData(executionHistoryQueryKey("workspace-1"), history);
  queryClient.setQueryData(cookieJarQueryKey("workspace-1"), cookies);
  requestApiMock.requestWorkspaceQuery.mockImplementation(
    ({ workspaceId }: { workspaceId: string }) => ({
      queryKey: requestWorkspaceQueryKey(workspaceId),
      queryFn: vi.fn().mockResolvedValue(snapshot),
    }),
  );
  requestApiMock.executionHistoryQuery.mockImplementation(
    ({ workspaceId }: { workspaceId: string }) => ({
      queryKey: executionHistoryQueryKey(workspaceId),
      queryFn: vi.fn().mockResolvedValue(history),
    }),
  );
  requestApiMock.cookieJarQuery.mockImplementation(
    ({ workspaceId }: { workspaceId: string }) => ({
      queryKey: cookieJarQueryKey(workspaceId),
      queryFn: vi.fn().mockResolvedValue(cookies),
    }),
  );

  render(
    <QueryClientProvider client={queryClient}>
      <App />
    </QueryClientProvider>,
  );

  return queryClient;
}

function replaceInputText(label: string, value: string) {
  const input = screen.getByLabelText(label);
  fireEvent.change(input, { target: { value } });
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, reject, resolve };
}

function workspaceSnapshot(): WorkspaceSnapshotDto {
  return {
    selectedWorkspaceId: "workspace-1",
    workspaces: [
      { id: "workspace-1", name: "Personal", isSelected: true, baseDirectory: null },
    ],
  };
}

function emptyRequestSnapshot(): RequestWorkspaceSnapshotDto {
  return {
    workspaceId: "workspace-1",
    collectionFolders: [],
    environments: [],
    collectionVariables: [],
    environmentVariables: [],
    savedRequests: [],
    drafts: [],
    tabs: [],
  };
}

function emptyExecutionHistorySnapshot(): ExecutionHistorySnapshotDto {
  return {
    workspaceId: "workspace-1",
    disabled: false,
    records: [],
    warning:
      "Unknown sensitive values inside arbitrary response bodies may not always be detected.",
  };
}

function executionHistorySnapshot({
  disabled = false,
  pinned = false,
}: {
  disabled?: boolean;
  pinned?: boolean;
} = {}): ExecutionHistorySnapshotDto {
  return {
    ...emptyExecutionHistorySnapshot(),
    disabled,
    records: [executionRecord({ pinned })],
  };
}

function executionRecord({ pinned = false }: { pinned?: boolean } = {}): ExecutionRecordDto {
  return {
    id: "history-1",
    workspaceId: "workspace-1",
    createdAtEpochSeconds: 1_800_000_000n,
    request: requestContent({
      name: "History Request",
      url: "https://history.example.test",
    }),
    response: {
      status: 200,
      headers: [],
      bodyPreview: "{\"ok\":true}",
      bodyTruncated: false,
      error: null,
      durationMs: 42n,
    },
    pinned,
  };
}

function emptyCookieJarSnapshot(): CookieJarSnapshotDto {
  return {
    workspaceId: "workspace-1",
    cookies: [],
  };
}

function cookieJarSnapshot(): CookieJarSnapshotDto {
  return {
    workspaceId: "workspace-1",
    cookies: [
      {
        id: "cookie-1",
        workspaceId: "workspace-1",
        name: "sid",
        domain: "example.test",
        path: "/",
        secure: true,
        httpOnly: true,
        sameSite: "LAX",
        expiresAtEpochSeconds: 1_900_000_000n,
        session: false,
        hasValue: true,
        valuePreview: "********",
      },
    ],
  };
}

function requestSnapshot({
  content = requestContent(),
  isDirty = false,
  savedRequestId = null,
  tabTitle = "Untitled Request",
}: {
  content?: RequestContentDto;
  isDirty?: boolean;
  savedRequestId?: string | null;
  tabTitle?: string;
} = {}): RequestWorkspaceSnapshotDto {
  return {
    workspaceId: "workspace-1",
    collectionFolders: [],
    environments: [],
    collectionVariables: [],
    environmentVariables: [],
    savedRequests: savedRequestId
      ? [
          {
            id: savedRequestId,
            workspaceId: "workspace-1",
            collectionId: null,
            position: 0,
            content,
          },
        ]
      : [],
    drafts: [
      {
        id: "draft-1",
        workspaceId: "workspace-1",
        savedRequestId,
        content,
        isDirty,
      },
    ],
    tabs: [
      {
        id: "tab-1",
        workspaceId: "workspace-1",
        savedRequestId,
        draftId: "draft-1",
        position: 0,
        title: tabTitle,
        isActive: true,
      },
    ],
  };
}

function twoTabRequestSnapshot(): RequestWorkspaceSnapshotDto {
  const first = requestContent({ name: "Untitled Request" });
  const second = requestContent({
    name: "Second Request",
    url: "https://example.test/second",
  });
  return {
    workspaceId: "workspace-1",
    collectionFolders: [],
    environments: [],
    collectionVariables: [],
    environmentVariables: [],
    savedRequests: [],
    drafts: [
      {
        id: "draft-1",
        workspaceId: "workspace-1",
        savedRequestId: null,
        content: first,
        isDirty: false,
      },
      {
        id: "draft-2",
        workspaceId: "workspace-1",
        savedRequestId: null,
        content: second,
        isDirty: false,
      },
    ],
    tabs: [
      {
        id: "tab-1",
        workspaceId: "workspace-1",
        savedRequestId: null,
        draftId: "draft-1",
        position: 0,
        title: "Untitled Request",
        isActive: true,
      },
      {
        id: "tab-2",
        workspaceId: "workspace-1",
        savedRequestId: null,
        draftId: "draft-2",
        position: 1,
        title: "Second Request",
        isActive: false,
      },
    ],
  };
}

function collectionRequestSnapshot({
  activeSavedRequestId = null,
  includeMoveTarget = false,
}: {
  activeSavedRequestId?: string | null;
  includeMoveTarget?: boolean;
} = {}): RequestWorkspaceSnapshotDto {
  const content = requestContent({ name: "Saved Request" });
  return {
    workspaceId: "workspace-1",
    collectionFolders: [
      {
        id: "collection-1",
        workspaceId: "workspace-1",
        parentCollectionId: null,
        name: "Folder",
        position: 0,
      },
      ...(includeMoveTarget
        ? [
            {
              id: "collection-2",
              workspaceId: "workspace-1",
              parentCollectionId: null,
              name: "Archive",
              position: 1,
            },
          ]
        : []),
    ],
    environments: [],
    collectionVariables: [],
    environmentVariables: [],
    savedRequests: [
      {
        id: "saved-1",
        workspaceId: "workspace-1",
        collectionId: "collection-1",
        position: 0,
        content,
      },
      ...(includeMoveTarget
        ? [
            {
              id: "saved-2",
              workspaceId: "workspace-1",
              collectionId: "collection-1",
              position: 1,
              content: requestContent({ name: "Later Request", method: "POST" }),
            },
          ]
        : []),
    ],
    drafts: activeSavedRequestId
      ? [
          {
            id: "draft-1",
            workspaceId: "workspace-1",
            savedRequestId: activeSavedRequestId,
            content,
            isDirty: false,
          },
        ]
      : [],
    tabs: activeSavedRequestId
      ? [
          {
            id: "tab-1",
            workspaceId: "workspace-1",
            savedRequestId: activeSavedRequestId,
            draftId: "draft-1",
            position: 0,
            title: "Saved Request",
            isActive: true,
          },
        ]
      : [],
  };
}

function environmentRequestSnapshot({
  selectedEnvironmentId = null,
}: {
  selectedEnvironmentId?: string | null;
} = {}): RequestWorkspaceSnapshotDto {
  const content = requestContent({
    url: "{{baseUrl}}/users",
    headers: [{ enabled: true, order: 0, name: "Authorization", value: "Bearer {{token}}" }],
  });
  return {
    ...requestSnapshot({ content, isDirty: false }),
    environments: [
      {
        id: "env-dev",
        workspaceId: "workspace-1",
        name: "Development",
        position: 0,
        isSelected: selectedEnvironmentId === "env-dev",
      },
      {
        id: "env-prod",
        workspaceId: "workspace-1",
        name: "Production",
        position: 1,
        isSelected: selectedEnvironmentId === "env-prod",
      },
    ],
    collectionVariables: [
      {
        workspaceId: "workspace-1",
        variable: {
          name: "baseUrl",
          value: { type: "PLAIN", value: "https://collection.example.test" },
        },
      },
    ],
    environmentVariables: [
      {
        environmentId: "env-prod",
        workspaceId: "workspace-1",
        variable: {
          name: "baseUrl",
          value: { type: "PLAIN", value: "https://prod.example.test" },
        },
      },
      {
        environmentId: "env-prod",
        workspaceId: "workspace-1",
        variable: {
          name: "token",
          value: { type: "SECRET_REFERENCE", reference: "secret://token-prod" },
        },
      },
    ],
  };
}

function emptyResolution() {
  return {
    url: { value: "", containsSecret: false },
    body: { value: "", containsSecret: false },
    query: [],
    headers: [],
    unsafeTlsVisible: false,
    references: [],
    errors: [],
  };
}

function requestContent(
  overrides: Partial<RequestContentDto> = {},
): RequestContentDto {
  const url = overrides.url ?? "https://example.test";
  return {
    name: "Untitled Request",
    method: "GET",
    url,
    body: { type: "NONE" },
    query: queryFromUrl(url),
    headers: [],
    auth: { type: "NONE" },
    redirect: { enabled: true, maxRedirects: 10 },
    tls: {
      verify: true,
      customCaReference: null,
      clientCertificateReference: null,
      clientKeyReference: null,
    },
    transport: defaultTransport(),
    ...overrides,
  };
}

type ExecutionEventKindInput =
  | ({ type: "STARTED" } & Partial<Extract<ExecutionEventKindDto, { type: "STARTED" }>>)
  | ({ type: "RESPONSE_HEADERS" } & Partial<Extract<ExecutionEventKindDto, { type: "RESPONSE_HEADERS" }>>)
  | ({ type: "COMPLETED" } & Partial<Extract<ExecutionEventKindDto, { type: "COMPLETED" }>>)
  | Extract<ExecutionEventKindDto, { type: "REDIRECTED" | "UPLOAD_PROGRESS" | "DOWNLOAD_PROGRESS" | "FAILED" | "CANCELLED" }>;

function executionEvent(
  executionId: string,
  sequence: bigint,
  kind: ExecutionEventKindInput,
): ExecutionEventDto {
  const completedKind = completeExecutionEventKind(kind);
  return {
    executionId,
    sequence,
    kind: completedKind,
  };
}

function completeExecutionEventKind(kind: ExecutionEventKindInput): ExecutionEventKindDto {
  if (kind.type === "STARTED") {
    return {
      method: "GET",
      url: "https://example.test",
      tlsVerification: true,
      proxy: defaultProxyMetadata(),
      timeouts: defaultTimeoutMetadata(),
      queuedMs: 0n,
      ...kind,
    };
  }
  if (kind.type === "RESPONSE_HEADERS") {
    return {
      status: 200,
      headers: [],
      protocol: "HTTP/1.1",
      remoteAddr: "127.0.0.1:8080",
      ...kind,
    };
  }
  if (kind.type === "COMPLETED") {
    return {
      status: 200,
      bodyPreview: "",
      bodyTruncated: false,
      decodedBytes: 0n,
      wireBytes: null,
      responseFile: null,
      timing: defaultTimingMetadata(),
      ...kind,
    };
  }
  return kind;
}

function defaultTransport() {
  return {
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
  };
}

function defaultProxyMetadata() {
  return {
    source: "processEnvironment",
    selectedProxy: null,
    bypassReason: null,
  };
}

function defaultTimingMetadata() {
  return {
    queuedMs: 0n,
    dnsMs: null,
    connectMs: null,
    tlsMs: null,
    firstByteMs: 12n,
    downloadMs: 3n,
    totalMs: 20n,
  };
}

function defaultTimeoutMetadata() {
  return {
    connectMs: 10_000n,
    overallMs: 300_000n,
    idleMs: 60_000n,
  };
}
