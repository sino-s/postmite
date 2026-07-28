import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  ArrowDown,
  ArrowUp,
  Ban,
  Cookie,
  Copy,
  Edit3,
  FileText,
  Folder,
  FolderPlus,
  History,
  Pin,
  PinOff,
  Play,
  Plus,
  RotateCcw,
  Save,
  Trash2,
  X,
} from "lucide-react";
import { useEffect, useMemo, useState } from "react";

import {
  closeRequestTab,
  createCollectionFolder,
  clearCookies,
  cookieJarQuery,
  deleteCollectionFolder,
  deleteCookie,
  deleteSavedRequest,
  duplicateCollectionFolder,
  duplicateSavedRequest,
  executionHistoryQuery,
  executionHistoryQueryKey,
  moveCollectionFolder,
  moveSavedRequest,
  openExecutionRecordAsDraft,
  openSavedRequestTab,
  openUnsavedRequestTab,
  requestWorkspaceQuery,
  revealCookieValue,
  resolveRequestContent,
  renameCollectionFolder,
  saveRequestDraft,
  selectEnvironment,
  setExecutionHistoryDisabled,
  setExecutionRecordPinned,
  upsertCookie,
  updateRequestDraft,
} from "../../shared/api/requests";
import {
  cancelRequestExecution,
  createQueuedResponseExecutionState,
  isTerminalResponseExecution,
  listenToRequestExecutionEvents,
  reduceResponseExecutionStates,
  startRequestExecution,
} from "../../shared/api/execution";
import {
  workspaceQuery,
  workspaceQueryKey,
} from "../../shared/api/workspaces";
import type { ResponseExecutionState } from "../../shared/api/execution";
import type {
  OrderedFieldDto,
  CookieSameSiteDto,
  WorkspaceCookieDto,
  CollectionFolderDto,
  EnvironmentDto,
  ExecutionHistorySnapshotDto,
  ExecutionRecordDto,
  ResolvedRequestContentDto,
  RequestContentDto,
  RequestDraftDto,
  SavedRequestDto,
  RequestTabDto,
} from "../../shared/api/generated/ipc";
import { RawBodyEditor } from "./RawBodyEditor";
import {
  applyQueryToUrl,
  createEmptyField,
  normalizeFieldOrders,
  queryFromUrl,
  sortOrderedFields,
} from "./ordered-fields";

const METHODS = ["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS"];

type RequestEditorProps = {
  onCancel?: typeof cancelRequestExecution;
  onExecute?: typeof startRequestExecution;
};

type OverrideMap = Record<string, RequestContentDto>;

export function RequestEditor({
  onCancel = cancelRequestExecution,
  onExecute = startRequestExecution,
}: RequestEditorProps) {
  const queryClient = useQueryClient();
  const [activeTabId, setActiveTabId] = useState<string | null>(null);
  const [overrides, setOverrides] = useState<OverrideMap>({});
  const [executions, setExecutions] = useState<
    Record<string, ResponseExecutionState>
  >({});

  const workspaces = useQuery(workspaceQuery);
  const selectedWorkspaceId = workspaces.data?.selectedWorkspaceId;
  const requestWorkspace = useQuery({
    ...requestWorkspaceQuery({ workspaceId: selectedWorkspaceId ?? "" }),
    enabled: Boolean(selectedWorkspaceId),
  });
  const executionHistory = useQuery({
    ...executionHistoryQuery({ workspaceId: selectedWorkspaceId ?? "" }),
    enabled: Boolean(selectedWorkspaceId),
  });
  const cookieJar = useQuery({
    ...cookieJarQuery({ workspaceId: selectedWorkspaceId ?? "" }),
    enabled: Boolean(selectedWorkspaceId),
  });

  const snapshot = requestWorkspace.data;
  const tabs = useMemo(
    () => [...(snapshot?.tabs ?? [])].sort((left, right) => left.position - right.position),
    [snapshot?.tabs],
  );
  const activeTab =
    tabs.find((tab) => tab.id === activeTabId) ??
    tabs.find((tab) => tab.isActive) ??
    tabs[0] ??
    null;
  const activeDraft =
    activeTab && snapshot
      ? snapshot.drafts.find((draft) => draft.id === activeTab.draftId) ?? null
      : null;
  const activeContent =
    activeDraft ? overrides[activeDraft.id] ?? activeDraft.content : null;
  const selectedEnvironment =
    snapshot?.environments.find((environment) => environment.isSelected) ?? null;
  const activeExecution = activeDraft ? executions[activeDraft.id] ?? null : null;
  const activeExecutionRunning =
    activeExecution !== null && !isTerminalResponseExecution(activeExecution);
  const resolution = useQuery({
    queryKey: [
      "requestResolution",
      selectedWorkspaceId,
      activeDraft?.id ?? null,
      selectedEnvironment?.id ?? null,
      activeContent,
    ],
    queryFn: () =>
      resolveRequestContent({
        workspaceId: selectedWorkspaceId ?? "",
        content: activeContent ?? emptyRequestContent(),
      }),
    enabled: Boolean(selectedWorkspaceId && activeDraft && activeContent),
  });

  useEffect(() => {
    let unlisten: (() => void) | null = null;
    let disposed = false;

    void listenToRequestExecutionEvents((event) => {
      setExecutions((current) =>
        reduceResponseExecutionStates(current, event, Date.now()),
      );
      if (
        selectedWorkspaceId &&
        (event.kind.type === "COMPLETED" ||
          event.kind.type === "FAILED" ||
          event.kind.type === "CANCELLED")
      ) {
        void queryClient.invalidateQueries({
          queryKey: executionHistoryQueryKey(selectedWorkspaceId),
        });
      }
    }).then((nextUnlisten) => {
      if (disposed) {
        nextUnlisten();
        return;
      }
      unlisten = nextUnlisten;
    });

    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [queryClient, selectedWorkspaceId]);

  const openTabMutation = useMutation({
    mutationFn: async () => {
      if (!selectedWorkspaceId) {
        throw new Error("No workspace is selected.");
      }
      return openUnsavedRequestTab(queryClient, {
        workspaceId: selectedWorkspaceId,
      });
    },
    onSuccess: (nextSnapshot) => {
      const nextActive =
        nextSnapshot.tabs.find((tab) => tab.isActive) ??
        nextSnapshot.tabs[nextSnapshot.tabs.length - 1];
      setActiveTabId(nextActive?.id ?? null);
    },
  });

  async function handleCreateCollectionFolder(parentCollectionId: string | null) {
    if (!selectedWorkspaceId) {
      return;
    }
    const name = window.prompt("Folder name", "New Folder")?.trim();
    if (!name) {
      return;
    }
    await createCollectionFolder(queryClient, {
      workspaceId: selectedWorkspaceId,
      parentCollectionId,
      name,
    });
  }

  async function handleSelectEnvironment(environmentId: string | null) {
    if (!selectedWorkspaceId) {
      return;
    }
    await selectEnvironment(queryClient, {
      workspaceId: selectedWorkspaceId,
      environmentId,
    });
  }

  async function handleRenameCollectionFolder(folder: CollectionFolderDto) {
    const name = window.prompt("Folder name", folder.name)?.trim();
    if (!name || name === folder.name) {
      return;
    }
    await renameCollectionFolder(queryClient, {
      workspaceId: folder.workspaceId,
      collectionId: folder.id,
      name,
    });
  }

  async function handleMoveCollectionFolder(
    folder: CollectionFolderDto,
    direction: -1 | 1,
  ) {
    await moveCollectionFolder(queryClient, {
      workspaceId: folder.workspaceId,
      collectionId: folder.id,
      location: {
        collectionId: folder.parentCollectionId,
        position: Math.max(0, folder.position + direction),
      },
    });
  }

  async function handleDuplicateCollectionFolder(folder: CollectionFolderDto) {
    await duplicateCollectionFolder(queryClient, {
      workspaceId: folder.workspaceId,
      collectionId: folder.id,
    });
  }

  async function handleDeleteCollectionFolder(folder: CollectionFolderDto) {
    await deleteCollectionFolder(queryClient, {
      workspaceId: folder.workspaceId,
      collectionId: folder.id,
    });
  }

  async function handleOpenSavedRequest(request: SavedRequestDto) {
    const nextSnapshot = await openSavedRequestTab(queryClient, {
      workspaceId: request.workspaceId,
      savedRequestId: request.id,
    });
    setActiveTabId(
      nextSnapshot.tabs.find((tab) => tab.savedRequestId === request.id)?.id ??
        activeTabId,
    );
  }

  async function handleMoveSavedRequest(request: SavedRequestDto, direction: -1 | 1) {
    await moveSavedRequest(queryClient, {
      workspaceId: request.workspaceId,
      savedRequestId: request.id,
      location: {
        collectionId: request.collectionId,
        position: Math.max(0, request.position + direction),
      },
    });
  }

  async function handleDuplicateSavedRequest(request: SavedRequestDto) {
    await duplicateSavedRequest(queryClient, {
      workspaceId: request.workspaceId,
      savedRequestId: request.id,
    });
  }

  async function handleDeleteSavedRequest(request: SavedRequestDto) {
    await deleteSavedRequest(queryClient, {
      workspaceId: request.workspaceId,
      savedRequestId: request.id,
    });
  }

  async function persistDraft(draft: RequestDraftDto, content: RequestContentDto) {
    await updateRequestDraft({
      workspaceId: draft.workspaceId,
      draftId: draft.id,
      content,
    });
  }

  function changeActiveDraft(updater: (content: RequestContentDto) => RequestContentDto) {
    if (!activeDraft || !activeContent) {
      return;
    }

    const nextContent = updater(activeContent);
    setOverrides((current) => ({
      ...current,
      [activeDraft.id]: nextContent,
    }));
    void persistDraft(activeDraft, nextContent);
  }

  async function handleSave() {
    if (!activeDraft || !activeContent) {
      return;
    }

    await persistDraft(activeDraft, activeContent);
    const nextSnapshot = await saveRequestDraft(queryClient, {
      workspaceId: activeDraft.workspaceId,
      draftId: activeDraft.id,
    });
    setOverrides((current) => omitKey(current, activeDraft.id));
    setActiveTabId(
      nextSnapshot.tabs.find((tab) => tab.draftId === activeDraft.id)?.id ?? activeTabId,
    );
  }

  async function handleClose(tab: RequestTabDto, decision: "SAVE" | "DISCARD") {
    const draft = snapshot?.drafts.find((item) => item.id === tab.draftId);
    const content = draft ? overrides[draft.id] ?? draft.content : null;
    if (draft && content && decision === "SAVE") {
      await persistDraft(draft, content);
    }

    const nextSnapshot = await closeRequestTab(queryClient, {
      workspaceId: tab.workspaceId,
      tabId: tab.id,
      decision,
    });
    setOverrides((current) => omitKey(current, tab.draftId));
    setActiveTabId(
      nextSnapshot.tabs.find((item) => item.isActive)?.id ??
        nextSnapshot.tabs[0]?.id ??
        null,
    );
  }

  async function handleExecute() {
    if (!activeDraft || !activeContent) {
      return;
    }

    await persistDraft(activeDraft, activeContent);
    const result = await onExecute({
      workspaceId: activeDraft.workspaceId,
      draftId: activeDraft.id,
      content: activeContent,
    });
    setExecutions((current) => ({
      ...current,
      [activeDraft.id]: createQueuedResponseExecutionState({
        draftId: activeDraft.id,
        executionId: result.executionId,
        nowMs: Date.now(),
      }),
    }));
  }

  async function handleCancel() {
    if (!activeExecution || isTerminalResponseExecution(activeExecution)) {
      return;
    }

    const result = await onCancel({ executionId: activeExecution.executionId });
    if (!result.cancelled) {
      setExecutions((current) => ({
        ...current,
        [activeExecution.draftId]: {
          ...activeExecution,
          phase: "failed",
          completedAtMs: Date.now(),
          error: "Execution was already finished.",
        },
      }));
    }
  }

  async function handleToggleHistoryDisabled(disabled: boolean) {
    if (!selectedWorkspaceId) {
      return;
    }
    await setExecutionHistoryDisabled(queryClient, {
      workspaceId: selectedWorkspaceId,
      disabled,
    });
  }

  async function handleToggleHistoryPinned(record: ExecutionRecordDto) {
    await setExecutionRecordPinned(queryClient, {
      workspaceId: record.workspaceId,
      recordId: record.id,
      pinned: !record.pinned,
    });
  }

  async function handleUpsertCookie(input: CookieFormValue) {
    if (!selectedWorkspaceId) {
      return;
    }
    await upsertCookie(queryClient, {
      workspaceId: selectedWorkspaceId,
      cookieId: input.cookieId,
      name: input.name,
      value: input.value,
      domain: input.domain,
      path: input.path,
      secure: input.secure,
      httpOnly: input.httpOnly,
      sameSite: input.sameSite,
      expiresAtEpochSeconds: input.expiresAtEpochSeconds,
    });
  }

  async function handleDeleteCookie(cookie: WorkspaceCookieDto) {
    await deleteCookie(queryClient, {
      workspaceId: cookie.workspaceId,
      cookieId: cookie.id,
    });
  }

  async function handleClearCookies() {
    if (!selectedWorkspaceId) {
      return;
    }
    await clearCookies(queryClient, { workspaceId: selectedWorkspaceId });
  }

  async function handleRevealCookie(cookie: WorkspaceCookieDto) {
    return revealCookieValue({
      workspaceId: cookie.workspaceId,
      cookieId: cookie.id,
    });
  }

  async function handleOpenHistoryRecord(record: ExecutionRecordDto) {
    const nextSnapshot = await openExecutionRecordAsDraft(queryClient, {
      workspaceId: record.workspaceId,
      recordId: record.id,
    });
    setActiveTabId(
      nextSnapshot.tabs.find((tab) => tab.isActive)?.id ??
        nextSnapshot.tabs[nextSnapshot.tabs.length - 1]?.id ??
        activeTabId,
    );
  }

  function handleEditorKeyDown(event: React.KeyboardEvent<HTMLElement>) {
    if (!(event.ctrlKey || event.metaKey)) {
      return;
    }

    if (event.key === "s") {
      event.preventDefault();
      void handleSave();
    }
    if (event.key === "Enter") {
      event.preventDefault();
      void handleExecute();
    }
  }

  if (workspaces.isPending || requestWorkspace.isPending) {
    return (
      <main className="flex min-h-screen items-center justify-center bg-slate-100 text-slate-950">
        <p className="text-sm">Loading Postmite</p>
      </main>
    );
  }

  if (workspaces.isError || requestWorkspace.isError || !selectedWorkspaceId) {
    return (
      <main className="flex min-h-screen items-center justify-center bg-slate-100 p-6 text-slate-950">
        <section
          aria-labelledby="request-editor-error"
          className="w-full max-w-xl rounded-md border border-red-300 bg-white p-5"
        >
          <h1 id="request-editor-error" className="text-base font-semibold">
            Request workspace unavailable
          </h1>
          <button
            className="mt-4 inline-flex items-center gap-2 rounded-md border border-slate-300 bg-white px-3 py-2 text-sm font-medium hover:bg-slate-50 focus-visible:outline focus-visible:outline-2 focus-visible:outline-sky-500"
            onClick={() => {
              void queryClient.invalidateQueries({ queryKey: workspaceQueryKey });
            }}
            type="button"
          >
            <RotateCcw aria-hidden="true" size={16} />
            Retry
          </button>
        </section>
      </main>
    );
  }

  return (
    <main
      className="flex min-h-screen flex-col bg-slate-100 text-slate-950"
      onKeyDown={handleEditorKeyDown}
    >
      <header className="flex min-h-12 items-center justify-between border-b border-slate-300 bg-white px-4">
        <h1 className="text-sm font-semibold">Postmite</h1>
        <button
          className="inline-flex h-8 items-center gap-2 rounded-md bg-slate-900 px-3 text-sm font-medium text-white hover:bg-slate-700 focus-visible:outline focus-visible:outline-2 focus-visible:outline-sky-500 disabled:cursor-not-allowed disabled:opacity-60"
          disabled={openTabMutation.isPending}
          onClick={() => openTabMutation.mutate()}
          type="button"
        >
          <Plus aria-hidden="true" size={16} />
          New
        </button>
      </header>

      <div className="grid min-h-0 flex-1 grid-cols-1 md:grid-cols-[280px_minmax(0,1fr)]">
        <CollectionsSidebar
          environments={snapshot?.environments ?? []}
          folders={snapshot?.collectionFolders ?? []}
          onCreateFolder={(parentCollectionId) =>
            void handleCreateCollectionFolder(parentCollectionId)
          }
          onDeleteFolder={(folder) => void handleDeleteCollectionFolder(folder)}
          onDeleteRequest={(request) => void handleDeleteSavedRequest(request)}
          onDuplicateFolder={(folder) => void handleDuplicateCollectionFolder(folder)}
          onDuplicateRequest={(request) => void handleDuplicateSavedRequest(request)}
          onMoveFolder={(folder, direction) =>
            void handleMoveCollectionFolder(folder, direction)
          }
          onMoveRequest={(request, direction) =>
            void handleMoveSavedRequest(request, direction)
          }
          onOpenRequest={(request) => void handleOpenSavedRequest(request)}
          onRenameFolder={(folder) => void handleRenameCollectionFolder(folder)}
          onSelectEnvironment={(environmentId) =>
            void handleSelectEnvironment(environmentId)
          }
          requests={snapshot?.savedRequests ?? []}
        />
        <div className="flex min-h-0 flex-col">
          <TabStrip
            activeTabId={activeTab?.id ?? null}
            drafts={snapshot?.drafts ?? []}
            onActivate={setActiveTabId}
            onClose={(tab) => void handleClose(tab, isDraftDirty(tab.draftId, snapshot?.drafts ?? [], overrides) ? "SAVE" : "DISCARD")}
            tabs={tabs}
          />

          {activeDraft && activeContent ? (
            <section
              aria-label="Request editor"
              className="grid min-h-0 flex-1 grid-rows-[auto_minmax(0,1fr)_auto] gap-4 p-4"
            >
              <RequestLine
                content={activeContent}
                executionRunning={activeExecutionRunning}
                onCancel={() => void handleCancel()}
                onChange={changeActiveDraft}
                onExecute={() => void handleExecute()}
                onSave={() => void handleSave()}
                saving={false}
              />
              <div className="grid min-h-0 gap-4 lg:grid-cols-[minmax(0,1fr)_minmax(360px,0.9fr)]">
                <section className="flex min-h-0 flex-col gap-4">
                  <FieldTable
                    fields={activeContent.query}
                    legend="Params"
                    onChange={(fields) =>
                      changeActiveDraft((content) => ({
                        ...content,
                        query: fields,
                        url: applyQueryToUrl(content.url, fields),
                      }))
                    }
                  />
                  <FieldTable
                    fields={activeContent.headers}
                    legend="Headers"
                    onChange={(fields) =>
                      changeActiveDraft((content) => ({
                        ...content,
                        headers: fields,
                      }))
                    }
                  />
                </section>
                <RawBodyEditor
                  onChange={(body) =>
                    changeActiveDraft((content) => ({
                      ...content,
                      body,
                    }))
                  }
                  value={activeContent.body}
                />
              </div>
              <div className="grid gap-4 xl:grid-cols-[minmax(260px,0.4fr)_minmax(0,1fr)_minmax(300px,0.5fr)_minmax(300px,0.5fr)]">
                <ResolutionPanel
                  resolution={resolution.data ?? null}
                  resolving={resolution.isFetching}
                />
                <ResponsePanel execution={activeExecution} />
                <HistoryPanel
                  history={executionHistory.data ?? null}
                  loading={executionHistory.isFetching}
                  onOpen={(record) => void handleOpenHistoryRecord(record)}
                  onToggleDisabled={(disabled) =>
                    void handleToggleHistoryDisabled(disabled)
                  }
                  onTogglePinned={(record) =>
                    void handleToggleHistoryPinned(record)
                  }
                />
                <CookiePanel
                  cookies={cookieJar.data?.cookies ?? []}
                  loading={cookieJar.isFetching}
                  onClear={() => void handleClearCookies()}
                  onDelete={(cookie) => void handleDeleteCookie(cookie)}
                  onReveal={(cookie) => handleRevealCookie(cookie)}
                  onSave={(input) => void handleUpsertCookie(input)}
                />
              </div>
            </section>
          ) : (
            <section className="flex flex-1 items-center justify-center p-6">
              <button
                className="inline-flex h-10 items-center gap-2 rounded-md bg-slate-900 px-4 text-sm font-medium text-white hover:bg-slate-700 focus-visible:outline focus-visible:outline-2 focus-visible:outline-sky-500"
                onClick={() => openTabMutation.mutate()}
                type="button"
              >
                <Plus aria-hidden="true" size={16} />
                New Request
              </button>
            </section>
          )}
        </div>
      </div>
    </main>
  );
}

type TabStripProps = {
  activeTabId: string | null;
  drafts: RequestDraftDto[];
  onActivate: (tabId: string) => void;
  onClose: (tab: RequestTabDto) => void;
  tabs: RequestTabDto[];
};

type CollectionsSidebarProps = {
  environments: EnvironmentDto[];
  folders: CollectionFolderDto[];
  onCreateFolder: (parentCollectionId: string | null) => void;
  onDeleteFolder: (folder: CollectionFolderDto) => void;
  onDeleteRequest: (request: SavedRequestDto) => void;
  onDuplicateFolder: (folder: CollectionFolderDto) => void;
  onDuplicateRequest: (request: SavedRequestDto) => void;
  onMoveFolder: (folder: CollectionFolderDto, direction: -1 | 1) => void;
  onMoveRequest: (request: SavedRequestDto, direction: -1 | 1) => void;
  onOpenRequest: (request: SavedRequestDto) => void;
  onRenameFolder: (folder: CollectionFolderDto) => void;
  onSelectEnvironment: (environmentId: string | null) => void;
  requests: SavedRequestDto[];
};

type TreeRow =
  | {
      kind: "folder";
      id: string;
      depth: number;
      folder: CollectionFolderDto;
    }
  | {
      kind: "request";
      id: string;
      depth: number;
      request: SavedRequestDto;
    };

function CollectionsSidebar({
  environments,
  folders,
  onCreateFolder,
  onDeleteFolder,
  onDeleteRequest,
  onDuplicateFolder,
  onDuplicateRequest,
  onMoveFolder,
  onMoveRequest,
  onOpenRequest,
  onRenameFolder,
  onSelectEnvironment,
  requests,
}: CollectionsSidebarProps) {
  const rows = useMemo(
    () => buildCollectionRows(folders, requests, null, 0),
    [folders, requests],
  );

  function handleTreeKeyDown(
    event: React.KeyboardEvent<HTMLButtonElement>,
    row: TreeRow,
  ) {
    if (event.key === "Enter" && row.kind === "request") {
      event.preventDefault();
      onOpenRequest(row.request);
      return;
    }
    if (event.key !== "ArrowDown" && event.key !== "ArrowUp") {
      return;
    }

    event.preventDefault();
    const items = Array.from(
      event.currentTarget
        .closest("[data-collection-tree]")
        ?.querySelectorAll<HTMLButtonElement>("[data-tree-item]") ?? [],
    );
    const currentIndex = items.indexOf(event.currentTarget);
    const nextIndex =
      event.key === "ArrowDown"
        ? Math.min(items.length - 1, currentIndex + 1)
        : Math.max(0, currentIndex - 1);
    items[nextIndex]?.focus();
  }

  return (
    <aside
      aria-label="Collections"
      className="flex min-h-0 flex-col border-b border-slate-300 bg-slate-50 md:border-b-0 md:border-r"
    >
      <div className="flex h-11 shrink-0 items-center justify-between border-b border-slate-300 px-3">
        <h2 className="text-sm font-semibold">Collections</h2>
        <button
          aria-label="New root folder"
          className="inline-flex h-8 w-8 items-center justify-center rounded-md text-slate-700 hover:bg-white focus-visible:outline focus-visible:outline-2 focus-visible:outline-sky-500"
          onClick={() => onCreateFolder(null)}
          title="New root folder"
          type="button"
        >
          <FolderPlus aria-hidden="true" size={16} />
        </button>
      </div>
      <div className="border-b border-slate-300 px-3 py-3">
        <label className="mb-1 block text-xs font-semibold text-slate-600" htmlFor="environment-select">
          Environment
        </label>
        <select
          className="h-9 w-full rounded-md border border-slate-300 bg-white px-2 text-sm focus:border-sky-500 focus:outline focus:outline-2 focus:outline-sky-500"
          id="environment-select"
          onChange={(event) =>
            onSelectEnvironment(event.currentTarget.value || null)
          }
          value={environments.find((environment) => environment.isSelected)?.id ?? ""}
        >
          <option value="">No environment</option>
          {environments
            .slice()
            .sort(compareTreeItems)
            .map((environment) => (
              <option key={environment.id} value={environment.id}>
                {environment.name}
              </option>
            ))}
        </select>
      </div>
      <div
        aria-label="Collection tree"
        className="min-h-32 flex-1 overflow-auto py-2"
        data-collection-tree
        role={rows.length > 0 ? "tree" : undefined}
      >
        {rows.map((row) => {
          const label = row.kind === "folder" ? row.folder.name : row.request.content.name;
          return (
            <div
              className="group flex h-9 items-center gap-1 px-2"
              key={`${row.kind}-${row.id}`}
              role="none"
              style={{ paddingLeft: `${8 + row.depth * 16}px` }}
            >
              <button
                className="inline-flex min-w-0 flex-1 items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm hover:bg-white focus-visible:outline focus-visible:outline-2 focus-visible:outline-sky-500"
                data-tree-item
                onClick={() => {
                  if (row.kind === "request") {
                    onOpenRequest(row.request);
                  }
                }}
                onKeyDown={(event) => handleTreeKeyDown(event, row)}
                role="treeitem"
                type="button"
              >
                {row.kind === "folder" ? (
                  <Folder aria-hidden="true" className="shrink-0 text-amber-700" size={16} />
                ) : (
                  <FileText aria-hidden="true" className="shrink-0 text-sky-700" size={16} />
                )}
                <span className="truncate">{label}</span>
              </button>
              {row.kind === "folder" ? (
                <TreeActions
                  onCreate={() => onCreateFolder(row.folder.id)}
                  onDelete={() => onDeleteFolder(row.folder)}
                  onDuplicate={() => onDuplicateFolder(row.folder)}
                  onMoveDown={() => onMoveFolder(row.folder, 1)}
                  onMoveUp={() => onMoveFolder(row.folder, -1)}
                  onRename={() => onRenameFolder(row.folder)}
                />
              ) : (
                <RequestTreeActions
                  onDelete={() => onDeleteRequest(row.request)}
                  onDuplicate={() => onDuplicateRequest(row.request)}
                  onMoveDown={() => onMoveRequest(row.request, 1)}
                  onMoveUp={() => onMoveRequest(row.request, -1)}
                />
              )}
            </div>
          );
        })}
        {rows.length === 0 ? (
          <p className="px-3 py-6 text-sm text-slate-500">No saved requests</p>
        ) : null}
      </div>
    </aside>
  );
}

type ResolutionPanelProps = {
  resolution: ResolvedRequestContentDto | null;
  resolving: boolean;
};

function ResolutionPanel({ resolution, resolving }: ResolutionPanelProps) {
  const references = resolution?.references ?? [];
  const errors = resolution?.errors ?? [];

  return (
    <section
      aria-label="Variable resolution"
      className="min-h-40 rounded-md border border-slate-300 bg-white p-3 text-sm"
    >
      <div className="mb-3 flex items-center justify-between gap-3">
        <h2 className="text-sm font-semibold">Variables</h2>
        {resolving ? <span className="text-xs text-slate-500">Resolving</span> : null}
      </div>
      {errors.length > 0 ? (
        <div className="mb-3 rounded-md border border-red-200 bg-red-50 p-2 text-xs text-red-800">
          {errors.map((error) => (
            <p key={`${error.name}-${error.kind}`}>
              {error.name}: {formatResolutionError(error.kind)}
            </p>
          ))}
        </div>
      ) : null}
      <div className="overflow-x-auto rounded-md border border-slate-200">
        <table className="w-full min-w-[360px] table-fixed border-collapse text-left text-xs">
          <thead>
            <tr className="border-b border-slate-200 bg-slate-50 text-slate-600">
              <th className="w-32 px-2 py-2 font-semibold">Name</th>
              <th className="w-28 px-2 py-2 font-semibold">Source</th>
              <th className="px-2 py-2 font-semibold">Value</th>
            </tr>
          </thead>
          <tbody>
            {references.map((reference) => (
              <tr className="border-b border-slate-100" key={reference.name}>
                <td className="break-words px-2 py-2 font-medium text-slate-700">
                  {reference.name}
                </td>
                <td className="px-2 py-2 text-slate-600">
                  {formatVariableSource(reference.source)}
                </td>
                <td className="break-words px-2 py-2 text-slate-600">
                  {reference.value.value}
                </td>
              </tr>
            ))}
            {references.length === 0 ? (
              <tr>
                <td className="px-2 py-5 text-center text-slate-500" colSpan={3}>
                  No variables resolved
                </td>
              </tr>
            ) : null}
          </tbody>
        </table>
      </div>
    </section>
  );
}

type TreeActionsProps = {
  onCreate: () => void;
  onDelete: () => void;
  onDuplicate: () => void;
  onMoveDown: () => void;
  onMoveUp: () => void;
  onRename: () => void;
};

function TreeActions({
  onCreate,
  onDelete,
  onDuplicate,
  onMoveDown,
  onMoveUp,
  onRename,
}: TreeActionsProps) {
  return (
    <div className="flex shrink-0 items-center opacity-100 md:opacity-0 md:group-hover:opacity-100 md:focus-within:opacity-100">
      <IconButton label="New subfolder" onClick={onCreate}>
        <FolderPlus aria-hidden="true" size={14} />
      </IconButton>
      <IconButton label="Rename folder" onClick={onRename}>
        <Edit3 aria-hidden="true" size={14} />
      </IconButton>
      <IconButton label="Move folder up" onClick={onMoveUp}>
        <ArrowUp aria-hidden="true" size={14} />
      </IconButton>
      <IconButton label="Move folder down" onClick={onMoveDown}>
        <ArrowDown aria-hidden="true" size={14} />
      </IconButton>
      <IconButton label="Duplicate folder" onClick={onDuplicate}>
        <Copy aria-hidden="true" size={14} />
      </IconButton>
      <IconButton label="Delete folder" onClick={onDelete}>
        <Trash2 aria-hidden="true" size={14} />
      </IconButton>
    </div>
  );
}

type RequestTreeActionsProps = {
  onDelete: () => void;
  onDuplicate: () => void;
  onMoveDown: () => void;
  onMoveUp: () => void;
};

function RequestTreeActions({
  onDelete,
  onDuplicate,
  onMoveDown,
  onMoveUp,
}: RequestTreeActionsProps) {
  return (
    <div className="flex shrink-0 items-center opacity-100 md:opacity-0 md:group-hover:opacity-100 md:focus-within:opacity-100">
      <IconButton label="Move request up" onClick={onMoveUp}>
        <ArrowUp aria-hidden="true" size={14} />
      </IconButton>
      <IconButton label="Move request down" onClick={onMoveDown}>
        <ArrowDown aria-hidden="true" size={14} />
      </IconButton>
      <IconButton label="Duplicate request" onClick={onDuplicate}>
        <Copy aria-hidden="true" size={14} />
      </IconButton>
      <IconButton label="Delete request" onClick={onDelete}>
        <Trash2 aria-hidden="true" size={14} />
      </IconButton>
    </div>
  );
}

function IconButton({
  children,
  label,
  onClick,
}: {
  children: React.ReactNode;
  label: string;
  onClick: () => void;
}) {
  return (
    <button
      aria-label={label}
      className="inline-flex h-7 w-7 items-center justify-center rounded-md text-slate-600 hover:bg-white focus-visible:outline focus-visible:outline-2 focus-visible:outline-sky-500"
      onClick={onClick}
      title={label}
      type="button"
    >
      {children}
    </button>
  );
}

function buildCollectionRows(
  folders: CollectionFolderDto[],
  requests: SavedRequestDto[],
  parentCollectionId: string | null,
  depth: number,
): TreeRow[] {
  const childFolders = folders
    .filter((folder) => folder.parentCollectionId === parentCollectionId)
    .sort(compareTreeItems);
  const childRequests = requests
    .filter((request) => request.collectionId === parentCollectionId)
    .sort(compareTreeItems);

  return [
    ...childFolders.flatMap((folder) => [
      { kind: "folder" as const, id: folder.id, depth, folder },
      ...buildCollectionRows(folders, requests, folder.id, depth + 1),
    ]),
    ...childRequests.map((request) => ({
      kind: "request" as const,
      id: request.id,
      depth,
      request,
    })),
  ];
}

function compareTreeItems(
  left: Pick<CollectionFolderDto | SavedRequestDto, "position" | "id">,
  right: Pick<CollectionFolderDto | SavedRequestDto, "position" | "id">,
) {
  return left.position - right.position || left.id.localeCompare(right.id);
}

function TabStrip({
  activeTabId,
  drafts,
  onActivate,
  onClose,
  tabs,
}: TabStripProps) {
  return (
    <nav
      aria-label="Request tabs"
      className="flex min-h-11 items-stretch overflow-x-auto border-b border-slate-300 bg-white"
    >
      {tabs.map((tab) => {
        const dirty = isDraftDirty(tab.draftId, drafts, {});
        return (
          <div
            className="flex min-w-44 items-center border-r border-slate-300"
            key={tab.id}
          >
            <button
              aria-current={activeTabId === tab.id ? "page" : undefined}
              className="min-w-0 flex-1 px-3 py-2 text-left text-sm hover:bg-slate-100 focus-visible:outline focus-visible:outline-2 focus-visible:outline-sky-500 aria-[current=page]:bg-sky-50"
              onClick={() => onActivate(tab.id)}
              type="button"
            >
              <span className="block truncate">
                {tab.title}
                {dirty ? " *" : ""}
              </span>
            </button>
            <button
              aria-label={`Close ${tab.title}`}
              className="mr-1 inline-flex h-8 w-8 items-center justify-center rounded-md text-slate-600 hover:bg-slate-100 focus-visible:outline focus-visible:outline-2 focus-visible:outline-sky-500"
              onClick={() => onClose(tab)}
              type="button"
            >
              <X aria-hidden="true" size={16} />
            </button>
          </div>
        );
      })}
    </nav>
  );
}

type RequestLineProps = {
  content: RequestContentDto;
  executionRunning: boolean;
  onCancel: () => void;
  onChange: (updater: (content: RequestContentDto) => RequestContentDto) => void;
  onExecute: () => void;
  onSave: () => void;
  saving: boolean;
};

function RequestLine({
  content,
  executionRunning,
  onCancel,
  onChange,
  onExecute,
  onSave,
  saving,
}: RequestLineProps) {
  return (
    <div className="grid gap-3 rounded-md border border-slate-300 bg-white p-3 md:grid-cols-[180px_140px_minmax(0,1fr)_auto_auto_auto]">
      <label className="sr-only" htmlFor="request-name">
        Name
      </label>
      <input
        className="h-10 min-w-0 rounded-md border border-slate-300 px-3 text-sm focus:border-sky-500 focus:outline focus:outline-2 focus:outline-sky-500"
        id="request-name"
        onChange={(event) =>
          onChange((current) => ({
            ...current,
            name: event.currentTarget.value,
          }))
        }
        placeholder="Request name"
        value={content.name}
      />
      <label className="sr-only" htmlFor="request-method">
        Method
      </label>
      <select
        className="h-10 rounded-md border border-slate-300 bg-white px-3 text-sm font-medium focus:border-sky-500 focus:outline focus:outline-2 focus:outline-sky-500"
        id="request-method"
        onChange={(event) =>
          onChange((current) => ({
            ...current,
            method: event.currentTarget.value,
          }))
        }
        value={content.method}
      >
        {METHODS.map((method) => (
          <option key={method} value={method}>
            {method}
          </option>
        ))}
      </select>
      <label className="sr-only" htmlFor="request-url">
        URL
      </label>
      <input
        className="h-10 min-w-0 rounded-md border border-slate-300 px-3 text-sm focus:border-sky-500 focus:outline focus:outline-2 focus:outline-sky-500"
        id="request-url"
        onChange={(event) => {
          const url = event.currentTarget.value;
          onChange((current) => ({
            ...current,
            url,
            query: queryFromUrl(url),
          }));
        }}
        placeholder="https://example.test/resource?tag=one&tag="
        value={content.url}
      />
      <button
        className="inline-flex h-10 items-center justify-center gap-2 rounded-md border border-slate-300 bg-white px-3 text-sm font-medium hover:bg-slate-50 focus-visible:outline focus-visible:outline-2 focus-visible:outline-sky-500"
        disabled={saving}
        onClick={onSave}
        type="button"
      >
        <Save aria-hidden="true" size={16} />
        Save
      </button>
      <button
        className="inline-flex h-10 items-center justify-center gap-2 rounded-md bg-sky-700 px-3 text-sm font-semibold text-white hover:bg-sky-600 focus-visible:outline focus-visible:outline-2 focus-visible:outline-sky-500"
        disabled={executionRunning}
        onClick={onExecute}
        type="button"
      >
        <Play aria-hidden="true" size={16} />
        Send
      </button>
      <button
        className="inline-flex h-10 items-center justify-center gap-2 rounded-md border border-red-300 bg-white px-3 text-sm font-medium text-red-700 hover:bg-red-50 focus-visible:outline focus-visible:outline-2 focus-visible:outline-sky-500 disabled:cursor-not-allowed disabled:opacity-60"
        disabled={!executionRunning}
        onClick={onCancel}
        type="button"
      >
        <Ban aria-hidden="true" size={16} />
        Cancel
      </button>
    </div>
  );
}

type HistoryPanelProps = {
  history: ExecutionHistorySnapshotDto | null;
  loading: boolean;
  onOpen: (record: ExecutionRecordDto) => void;
  onToggleDisabled: (disabled: boolean) => void;
  onTogglePinned: (record: ExecutionRecordDto) => void;
};

function HistoryPanel({
  history,
  loading,
  onOpen,
  onToggleDisabled,
  onTogglePinned,
}: HistoryPanelProps) {
  const records = history?.records ?? [];

  return (
    <section
      aria-label="Execution history"
      className="grid min-h-40 gap-3 rounded-md border border-slate-300 bg-white p-3 text-sm"
    >
      <div className="flex items-center justify-between gap-3">
        <h2 className="inline-flex items-center gap-2 text-sm font-semibold">
          <History aria-hidden="true" size={16} />
          History
        </h2>
        {loading ? <span className="text-xs text-slate-500">Loading</span> : null}
      </div>
      {history ? (
        <label className="inline-flex items-center gap-2 text-xs font-medium text-slate-700">
          <input
            checked={history.disabled}
            className="h-4 w-4 rounded border-slate-300 text-slate-900 focus:ring-sky-500"
            onChange={(event) => onToggleDisabled(event.currentTarget.checked)}
            type="checkbox"
          />
          Disable history
        </label>
      ) : null}
      <p className="rounded-md border border-amber-200 bg-amber-50 px-2 py-2 text-xs text-amber-900">
        {history?.warning ??
          "Unknown sensitive values inside arbitrary response bodies may not always be detected."}
      </p>
      <div className="max-h-72 overflow-auto rounded-md border border-slate-200">
        {records.map((record) => (
          <div
            className="grid gap-2 border-b border-slate-100 p-2 last:border-b-0"
            key={record.id}
          >
            <div className="flex items-start justify-between gap-2">
              <button
                className="min-w-0 text-left focus-visible:outline focus-visible:outline-2 focus-visible:outline-sky-500"
                onClick={() => onOpen(record)}
                type="button"
              >
                <span className="block truncate text-sm font-semibold text-slate-900">
                  {record.request.name}
                </span>
                <span className="block truncate text-xs text-slate-600">
                  {record.request.method} {record.request.url}
                </span>
              </button>
              <button
                aria-label={record.pinned ? "Unpin history record" : "Pin history record"}
                className="inline-flex h-8 w-8 shrink-0 items-center justify-center rounded-md text-slate-600 hover:bg-slate-100 focus-visible:outline focus-visible:outline-2 focus-visible:outline-sky-500"
                onClick={() => onTogglePinned(record)}
                title={record.pinned ? "Unpin history record" : "Pin history record"}
                type="button"
              >
                {record.pinned ? (
                  <PinOff aria-hidden="true" size={15} />
                ) : (
                  <Pin aria-hidden="true" size={15} />
                )}
              </button>
            </div>
            <div className="flex flex-wrap gap-2 text-xs text-slate-600">
              <span>{formatHistoryTime(record.createdAtEpochSeconds)}</span>
              {record.response.status ? <span>Status {record.response.status}</span> : null}
              {record.response.durationMs !== null ? (
                <span>{record.response.durationMs.toString()} ms</span>
              ) : null}
              {record.response.error ? <span>{record.response.error}</span> : null}
            </div>
          </div>
        ))}
        {records.length === 0 ? (
          <p className="px-2 py-6 text-center text-sm text-slate-500">
            No execution history
          </p>
        ) : null}
      </div>
    </section>
  );
}

type CookieFormValue = {
  cookieId: string | null;
  name: string;
  value: string;
  domain: string;
  path: string;
  secure: boolean;
  httpOnly: boolean;
  sameSite: CookieSameSiteDto | null;
  expiresAtEpochSeconds: bigint | null;
};

type CookiePanelProps = {
  cookies: WorkspaceCookieDto[];
  loading: boolean;
  onClear: () => void;
  onDelete: (cookie: WorkspaceCookieDto) => void;
  onReveal: (cookie: WorkspaceCookieDto) => Promise<{ value: string }>;
  onSave: (input: CookieFormValue) => void;
};

function CookiePanel({
  cookies,
  loading,
  onClear,
  onDelete,
  onReveal,
  onSave,
}: CookiePanelProps) {
  const [draft, setDraft] = useState<CookieFormValue>(emptyCookieForm());
  const [revealed, setRevealed] = useState<Record<string, string>>({});

  function editCookie(cookie: WorkspaceCookieDto) {
    setDraft({
      cookieId: cookie.id,
      name: cookie.name,
      value: "",
      domain: cookie.domain,
      path: cookie.path,
      secure: cookie.secure,
      httpOnly: cookie.httpOnly,
      sameSite: cookie.sameSite,
      expiresAtEpochSeconds: cookie.expiresAtEpochSeconds,
    });
  }

  async function reveal(cookie: WorkspaceCookieDto) {
    const value = await onReveal(cookie);
    setRevealed((current) => ({
      ...current,
      [cookie.id]: value.value,
    }));
  }

  function handleSubmit(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    onSave(draft);
    setDraft(emptyCookieForm());
  }

  return (
    <section
      aria-label="Cookie jar"
      className="grid min-h-40 gap-3 rounded-md border border-slate-300 bg-white p-3 text-sm"
    >
      <div className="flex items-center justify-between gap-3">
        <h2 className="inline-flex items-center gap-2 text-sm font-semibold">
          <Cookie aria-hidden="true" size={16} />
          Cookies
        </h2>
        {loading ? <span className="text-xs text-slate-500">Loading</span> : null}
      </div>
      <form className="grid gap-2" onSubmit={handleSubmit}>
        <div className="grid gap-2 sm:grid-cols-2">
          <label className="grid gap-1 text-xs font-medium text-slate-700">
            Cookie name
            <input
              className="h-8 rounded-md border border-slate-300 px-2 text-sm focus:border-sky-500 focus:outline focus:outline-2 focus:outline-sky-500"
              onChange={(event) => {
                const name = event.currentTarget.value;
                setDraft((current) => ({ ...current, name }));
              }}
              required
              value={draft.name}
            />
          </label>
          <label className="grid gap-1 text-xs font-medium text-slate-700">
            Cookie value
            <input
              className="h-8 rounded-md border border-slate-300 px-2 text-sm focus:border-sky-500 focus:outline focus:outline-2 focus:outline-sky-500"
              onChange={(event) => {
                const value = event.currentTarget.value;
                setDraft((current) => ({ ...current, value }));
              }}
              required
              type="password"
              value={draft.value}
            />
          </label>
          <label className="grid gap-1 text-xs font-medium text-slate-700">
            Cookie domain
            <input
              className="h-8 rounded-md border border-slate-300 px-2 text-sm focus:border-sky-500 focus:outline focus:outline-2 focus:outline-sky-500"
              onChange={(event) => {
                const domain = event.currentTarget.value;
                setDraft((current) => ({ ...current, domain }));
              }}
              required
              value={draft.domain}
            />
          </label>
          <label className="grid gap-1 text-xs font-medium text-slate-700">
            Cookie path
            <input
              className="h-8 rounded-md border border-slate-300 px-2 text-sm focus:border-sky-500 focus:outline focus:outline-2 focus:outline-sky-500"
              onChange={(event) => {
                const path = event.currentTarget.value;
                setDraft((current) => ({ ...current, path }));
              }}
              required
              value={draft.path}
            />
          </label>
        </div>
        <div className="grid gap-2 sm:grid-cols-[1fr_auto_auto]">
          <label className="grid gap-1 text-xs font-medium text-slate-700">
            SameSite
            <select
              className="h-8 rounded-md border border-slate-300 bg-white px-2 text-sm focus:border-sky-500 focus:outline focus:outline-2 focus:outline-sky-500"
              onChange={(event) => {
                const sameSite = (event.currentTarget.value || null) as
                  | CookieSameSiteDto
                  | null;
                setDraft((current) => ({
                  ...current,
                  sameSite,
                }));
              }}
              value={draft.sameSite ?? ""}
            >
              <option value="">Unset</option>
              <option value="STRICT">Strict</option>
              <option value="LAX">Lax</option>
              <option value="NONE">None</option>
            </select>
          </label>
          <label className="inline-flex items-end gap-2 pb-1 text-xs font-medium text-slate-700">
            <input
              checked={draft.secure}
              className="h-4 w-4 rounded border-slate-300 text-slate-900 focus:ring-sky-500"
              onChange={(event) => {
                const secure = event.currentTarget.checked;
                setDraft((current) => ({ ...current, secure }));
              }}
              type="checkbox"
            />
            Secure
          </label>
          <label className="inline-flex items-end gap-2 pb-1 text-xs font-medium text-slate-700">
            <input
              checked={draft.httpOnly}
              className="h-4 w-4 rounded border-slate-300 text-slate-900 focus:ring-sky-500"
              onChange={(event) => {
                const httpOnly = event.currentTarget.checked;
                setDraft((current) => ({ ...current, httpOnly }));
              }}
              type="checkbox"
            />
            HttpOnly
          </label>
        </div>
        <div className="flex flex-wrap gap-2">
          <button
            className="inline-flex h-8 items-center gap-2 rounded-md bg-slate-900 px-3 text-xs font-medium text-white hover:bg-slate-700 focus-visible:outline focus-visible:outline-2 focus-visible:outline-sky-500"
            type="submit"
          >
            <Save aria-hidden="true" size={14} />
            {draft.cookieId ? "Update cookie" : "Add cookie"}
          </button>
          <button
            className="inline-flex h-8 items-center gap-2 rounded-md border border-slate-300 px-3 text-xs font-medium hover:bg-slate-50 focus-visible:outline focus-visible:outline-2 focus-visible:outline-sky-500"
            onClick={() => setDraft(emptyCookieForm())}
            type="button"
          >
            <RotateCcw aria-hidden="true" size={14} />
            Reset
          </button>
          <button
            className="inline-flex h-8 items-center gap-2 rounded-md border border-red-300 px-3 text-xs font-medium text-red-700 hover:bg-red-50 focus-visible:outline focus-visible:outline-2 focus-visible:outline-sky-500"
            onClick={onClear}
            type="button"
          >
            <Trash2 aria-hidden="true" size={14} />
            Clear cookies
          </button>
        </div>
      </form>
      <div className="max-h-72 overflow-auto rounded-md border border-slate-200">
        {cookies.map((cookie) => (
          <div className="grid gap-2 border-b border-slate-100 p-2 last:border-b-0" key={cookie.id}>
            <div className="flex items-start justify-between gap-2">
              <div className="min-w-0">
                <p className="truncate text-sm font-semibold text-slate-900">{cookie.name}</p>
                <p className="truncate text-xs text-slate-600">
                  {cookie.domain}{cookie.path} {cookie.secure ? "Secure" : "Insecure"}
                </p>
              </div>
              <div className="flex shrink-0 items-center">
                <IconButton label={`Inspect ${cookie.name} cookie`} onClick={() => void reveal(cookie)}>
                  <FileText aria-hidden="true" size={14} />
                </IconButton>
                <IconButton label={`Edit ${cookie.name} cookie`} onClick={() => editCookie(cookie)}>
                  <Edit3 aria-hidden="true" size={14} />
                </IconButton>
                <IconButton label={`Delete ${cookie.name} cookie`} onClick={() => onDelete(cookie)}>
                  <Trash2 aria-hidden="true" size={14} />
                </IconButton>
              </div>
            </div>
            <div className="grid gap-1 text-xs text-slate-600">
              <span>Value {revealed[cookie.id] ?? cookie.valuePreview}</span>
              <span>
                {cookie.session ? "Session" : "Persistent"}
                {cookie.sameSite ? ` SameSite ${formatSameSite(cookie.sameSite)}` : ""}
                {!cookie.hasValue ? " value unavailable until edited" : ""}
              </span>
            </div>
          </div>
        ))}
        {cookies.length === 0 ? (
          <p className="px-2 py-6 text-center text-sm text-slate-500">No cookies</p>
        ) : null}
      </div>
    </section>
  );
}

type ResponsePanelProps = {
  execution: ResponseExecutionState | null;
};

function ResponsePanel({ execution }: ResponsePanelProps) {
  if (!execution) {
    return (
      <section
        aria-label="Response"
        aria-live="polite"
        className="min-h-40 rounded-md border border-slate-300 bg-white p-3 text-sm text-slate-600"
      >
        No response yet.
      </section>
    );
  }

  const elapsedMs =
    (execution.completedAtMs ?? Date.now()) - execution.startedAtMs;
  const bodyPreview = formatBodyPreview(execution.bodyPreview);
  const phaseLabel = execution.phase[0].toUpperCase() + execution.phase.slice(1);

  return (
    <section
      aria-label="Response"
      aria-live="polite"
      className="grid min-h-40 gap-3 rounded-md border border-slate-300 bg-white p-3 text-sm"
    >
      <div className="flex flex-wrap items-center gap-3">
        <span className="rounded-md border border-slate-300 px-2 py-1 font-medium text-slate-700">
          {phaseLabel}
        </span>
        {execution.status ? (
          <span className="font-semibold text-slate-950">
            Status {execution.status}
          </span>
        ) : null}
        <span className="text-slate-600">Time {Math.max(0, elapsedMs)} ms</span>
        {execution.downloadProgress ? (
          <span className="text-slate-600">
            Received {execution.downloadProgress.receivedBytes.toString()} bytes
          </span>
        ) : null}
        {execution.uploadProgress ? (
          <span className="text-slate-600">
            Sent {execution.uploadProgress.sentBytes.toString()} bytes
          </span>
        ) : null}
      </div>

      {execution.error ? (
        <p className="rounded-md border border-red-200 bg-red-50 px-3 py-2 text-red-800">
          {execution.error}
        </p>
      ) : null}

      <div className="grid min-h-0 gap-3 lg:grid-cols-[minmax(260px,0.45fr)_minmax(0,1fr)]">
        <div className="min-h-0 overflow-auto rounded-md border border-slate-200">
          <table className="w-full table-fixed border-collapse text-left text-xs">
            <thead>
              <tr className="border-b border-slate-200 bg-slate-50 text-slate-600">
                <th className="w-40 px-2 py-2 font-semibold">Header</th>
                <th className="px-2 py-2 font-semibold">Value</th>
              </tr>
            </thead>
            <tbody>
              {execution.headers.map((header, index) => (
                <tr className="border-b border-slate-100" key={`${header.name}-${index}`}>
                  <td className="break-words px-2 py-2 font-medium text-slate-700">
                    {header.name}
                  </td>
                  <td className="break-words px-2 py-2 text-slate-600">
                    {header.value}
                  </td>
                </tr>
              ))}
              {execution.headers.length === 0 ? (
                <tr>
                  <td className="px-2 py-5 text-center text-slate-500" colSpan={2}>
                    No response headers
                  </td>
                </tr>
              ) : null}
            </tbody>
          </table>
        </div>
        <div className="min-h-0 rounded-md border border-slate-200 bg-slate-950 p-3 text-slate-50">
          <pre className="max-h-52 overflow-auto whitespace-pre-wrap break-words text-xs leading-5">
            {bodyPreview || "No response body"}
          </pre>
          {execution.bodyTruncated ? (
            <p className="mt-2 text-xs text-amber-200">Response preview truncated.</p>
          ) : null}
        </div>
      </div>
    </section>
  );
}

type FieldTableProps = {
  fields: OrderedFieldDto[];
  legend: "Params" | "Headers";
  onChange: (fields: OrderedFieldDto[]) => void;
};

function FieldTable({ fields, legend, onChange }: FieldTableProps) {
  const orderedFields = sortOrderedFields(fields);

  function updateField(
    index: number,
    updater: (field: OrderedFieldDto) => OrderedFieldDto,
  ) {
    const nextFields = orderedFields.map((field, fieldIndex) =>
      fieldIndex === index ? updater(field) : field,
    );
    onChange(normalizeFieldOrders(nextFields));
  }

  return (
    <fieldset className="min-h-0 rounded-md border border-slate-300 bg-white p-3">
      <div className="mb-3 flex items-center justify-between gap-3">
        <legend className="text-sm font-semibold text-slate-950">{legend}</legend>
        <button
          className="inline-flex h-8 items-center gap-2 rounded-md border border-slate-300 px-2 text-xs font-medium hover:bg-slate-50 focus-visible:outline focus-visible:outline-2 focus-visible:outline-sky-500"
          onClick={() =>
            onChange([
              ...orderedFields,
              createEmptyField(orderedFields.length),
            ])
          }
          type="button"
        >
          <Plus aria-hidden="true" size={14} />
          Add
        </button>
      </div>
      <div className="overflow-x-auto">
        <table className="w-full min-w-[560px] table-fixed border-collapse text-sm">
          <thead>
            <tr className="border-y border-slate-200 bg-slate-50 text-left text-xs font-semibold uppercase text-slate-600">
              <th className="w-14 px-2 py-2">On</th>
              <th className="px-2 py-2">Name</th>
              <th className="px-2 py-2">Value</th>
              <th className="w-12 px-2 py-2">
                <span className="sr-only">Actions</span>
              </th>
            </tr>
          </thead>
          <tbody>
            {orderedFields.map((field, index) => (
              <tr className="border-b border-slate-200" key={field.order}>
                <td className="px-2 py-2">
                  <input
                    aria-label={`${legend} row ${index + 1} enabled`}
                    checked={field.enabled}
                    className="h-4 w-4 rounded border-slate-300 text-sky-700 focus-visible:outline focus-visible:outline-2 focus-visible:outline-sky-500"
                    onChange={(event) =>
                      updateField(index, (current) => ({
                        ...current,
                        enabled: event.currentTarget.checked,
                      }))
                    }
                    type="checkbox"
                  />
                </td>
                <td className="px-2 py-2">
                  <input
                    aria-label={`${legend} row ${index + 1} name`}
                    className="h-9 w-full rounded-md border border-slate-300 px-2 focus:border-sky-500 focus:outline focus:outline-2 focus:outline-sky-500"
                    onChange={(event) =>
                      updateField(index, (current) => ({
                        ...current,
                        name: event.currentTarget.value,
                      }))
                    }
                    value={field.name}
                  />
                </td>
                <td className="px-2 py-2">
                  <input
                    aria-label={`${legend} row ${index + 1} value`}
                    className="h-9 w-full rounded-md border border-slate-300 px-2 focus:border-sky-500 focus:outline focus:outline-2 focus:outline-sky-500"
                    onChange={(event) =>
                      updateField(index, (current) => ({
                        ...current,
                        value: event.currentTarget.value,
                      }))
                    }
                    value={field.value}
                  />
                </td>
                <td className="px-2 py-2">
                  <button
                    aria-label={`Remove ${legend} row ${index + 1}`}
                    className="inline-flex h-8 w-8 items-center justify-center rounded-md text-slate-600 hover:bg-slate-100 focus-visible:outline focus-visible:outline-2 focus-visible:outline-sky-500"
                    onClick={() =>
                      onChange(
                        normalizeFieldOrders(
                          orderedFields.filter((_, fieldIndex) => fieldIndex !== index),
                        ),
                      )
                    }
                    type="button"
                  >
                    <Trash2 aria-hidden="true" size={15} />
                  </button>
                </td>
              </tr>
            ))}
            {orderedFields.length === 0 ? (
              <tr>
                <td className="px-2 py-5 text-center text-sm text-slate-500" colSpan={4}>
                  No {legend.toLowerCase()}
                </td>
              </tr>
            ) : null}
          </tbody>
        </table>
      </div>
    </fieldset>
  );
}

function isDraftDirty(
  draftId: string,
  drafts: RequestDraftDto[],
  overrides: OverrideMap,
) {
  const draft = drafts.find((item) => item.id === draftId);
  return Boolean(draft?.isDirty || overrides[draftId]);
}

function omitKey<T>(record: Record<string, T>, key: string) {
  const next = { ...record };
  delete next[key];
  return next;
}

function formatBodyPreview(value: string) {
  const trimmed = value.trim();
  if (!trimmed) {
    return "";
  }

  try {
    return JSON.stringify(JSON.parse(trimmed), null, 2);
  } catch {
    return value;
  }
}

function formatHistoryTime(epochSeconds: bigint) {
  const timestamp = new Date(Number(epochSeconds) * 1000);
  if (Number.isNaN(timestamp.getTime())) {
    return "";
  }
  return timestamp.toLocaleString();
}

function emptyCookieForm(): CookieFormValue {
  return {
    cookieId: null,
    name: "",
    value: "",
    domain: "",
    path: "/",
    secure: false,
    httpOnly: false,
    sameSite: null,
    expiresAtEpochSeconds: null,
  };
}

function formatSameSite(value: CookieSameSiteDto) {
  switch (value) {
    case "STRICT":
      return "Strict";
    case "LAX":
      return "Lax";
    case "NONE":
      return "None";
  }
}

function emptyRequestContent(): RequestContentDto {
  return {
    name: "Untitled Request",
    method: "GET",
    url: "",
    body: "",
    query: [],
    headers: [],
  };
}

function formatVariableSource(source: string) {
  return source === "ENVIRONMENT" ? "Environment" : "Collection";
}

function formatResolutionError(kind: string) {
  return kind === "CYCLE" ? "Cyclic reference" : "Missing reference";
}
