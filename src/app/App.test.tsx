import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { axe } from "jest-axe";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type {
  RequestContentDto,
  RequestWorkspaceSnapshotDto,
  WorkspaceSnapshotDto,
} from "../shared/api/generated/ipc";
import { requestWorkspaceQueryKey } from "../shared/api/requests";
import { workspaceQueryKey } from "../shared/api/workspaces";
import { App } from "./App";
import { queryFromUrl } from "../features/request-editor/ordered-fields";

const workspaceApiMock = vi.hoisted(() => ({
  workspaceQuery: {
    queryKey: ["workspaces"] as const,
    queryFn: vi.fn(),
  },
  workspaceQueryKey: ["workspaces"] as const,
}));

const requestApiMock = vi.hoisted(() => ({
  closeRequestTab: vi.fn(),
  openUnsavedRequestTab: vi.fn(),
  requestWorkspaceQuery: vi.fn(),
  requestWorkspaceQueryKey: (workspaceId: string) =>
    ["requestWorkspace", workspaceId] as const,
  saveRequestDraft: vi.fn(),
  updateRequestDraft: vi.fn(),
}));

const executionApiMock = vi.hoisted(() => ({
  startRequestExecution: vi.fn(),
}));

vi.mock("../shared/api/workspaces", () => workspaceApiMock);
vi.mock("../shared/api/requests", () => requestApiMock);
vi.mock("../shared/api/execution", () => executionApiMock);
vi.mock("../features/request-editor/CodeMirrorBodyEditor", () => ({
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
    workspaceApiMock.workspaceQuery.queryFn.mockResolvedValue(
      workspaceSnapshot(),
    );
    requestApiMock.requestWorkspaceQuery.mockImplementation(
      ({ workspaceId }: { workspaceId: string }) => ({
        queryKey: requestWorkspaceQueryKey(workspaceId),
        queryFn: vi.fn().mockResolvedValue(emptyRequestSnapshot()),
      }),
    );
    executionApiMock.startRequestExecution.mockResolvedValue({
      status: "queued",
      executionId: "execution-1",
    });
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
    await user.clear(screen.getByLabelText("Name"));
    await user.type(screen.getByLabelText("Name"), "Create user");
    await user.selectOptions(screen.getByLabelText("Method"), "POST");
    await user.clear(screen.getByLabelText("URL"));
    await user.type(
      screen.getByLabelText("URL"),
      "https://example.test/users?tag=first&tag=",
    );
    await user.click(screen.getByLabelText("Raw body editor"));
    await user.paste("{\"ok\":true}");
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
        body: "{\"ok\":true}",
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

  it("keeps URL query text and Params rows bidirectionally synchronized", async () => {
    const user = userEvent.setup();
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

    await user.clear(screen.getByLabelText("Params row 2 value"));
    await user.type(screen.getByLabelText("Params row 2 value"), "second");

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
      content: requestContent(),
    });
    expect(
      await screen.findByText("Execution queued: execution-1"),
    ).toBeInTheDocument();
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

  it("has no automated accessibility violations in the editor shell", async () => {
    renderApp(
      requestSnapshot({ content: requestContent(), isDirty: true }),
    );

    await screen.findByLabelText("Request editor");
    const results = await axe(document.body);

    expect(results.violations).toEqual([]);
  });
});

function renderApp(snapshot: RequestWorkspaceSnapshotDto) {
  const queryClient = new QueryClient({
    defaultOptions: {
      queries: { retry: false },
      mutations: { retry: false },
    },
  });
  queryClient.setQueryData(workspaceQueryKey, workspaceSnapshot());
  queryClient.setQueryData(requestWorkspaceQueryKey("workspace-1"), snapshot);
  requestApiMock.requestWorkspaceQuery.mockImplementation(
    ({ workspaceId }: { workspaceId: string }) => ({
      queryKey: requestWorkspaceQueryKey(workspaceId),
      queryFn: vi.fn().mockResolvedValue(snapshot),
    }),
  );

  render(
    <QueryClientProvider client={queryClient}>
      <App />
    </QueryClientProvider>,
  );

  return queryClient;
}

function workspaceSnapshot(): WorkspaceSnapshotDto {
  return {
    selectedWorkspaceId: "workspace-1",
    workspaces: [{ id: "workspace-1", name: "Personal", isSelected: true }],
  };
}

function emptyRequestSnapshot(): RequestWorkspaceSnapshotDto {
  return {
    workspaceId: "workspace-1",
    savedRequests: [],
    drafts: [],
    tabs: [],
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
    savedRequests: savedRequestId
      ? [
          {
            id: savedRequestId,
            workspaceId: "workspace-1",
            collectionId: null,
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

function requestContent(
  overrides: Partial<RequestContentDto> = {},
): RequestContentDto {
  const url = overrides.url ?? "https://example.test";
  return {
    name: "Untitled Request",
    method: "GET",
    url,
    body: "",
    query: queryFromUrl(url),
    headers: [],
    ...overrides,
  };
}
