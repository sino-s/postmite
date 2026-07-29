import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Bug, Folder, Plus, RotateCcw } from "lucide-react";
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
  relinkBodyFiles,
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
  setWorkspaceBaseDirectory,
} from "../../shared/api/workspaces";
import { checkForUpdate } from "../../shared/api/update";
import type { ResponseExecutionState } from "../../shared/api/execution";
import type {
  WorkspaceCookieDto,
  CollectionFolderDto,
  ExecutionRecordDto,
  RequestContentDto,
  RequestDraftDto,
  SavedRequestDto,
  RequestTabDto,
} from "../../shared/api/generated/ipc";
import { BodyEditor } from "./components/BodyEditor";
import { CollectionsSidebar } from "./components/CollectionsSidebar";
import { CookiePanel } from "./components/CookiePanel";
import { DiagnosticsPanel } from "./components/DiagnosticsPanel";
import { FieldTable } from "./components/FieldTable";
import { HistoryPanel } from "./components/HistoryPanel";
import { RequestLine } from "./components/RequestLine";
import { ResolutionPanel } from "./components/ResolutionPanel";
import { ResponsePanel } from "./components/ResponsePanel";
import { SecurityPanel } from "./components/SecurityPanel";
import { TabStrip } from "./components/TabStrip";
import { applyQueryToUrl } from "./ordered-fields";
import { useI18n, type AppLocale } from "../../app/i18n";
import { usePreferences, type Density, type Theme } from "../../app/preferences";
import {
  emptyRequestContent,
  isDraftDirty,
  omitKey,
  requestContentQueryKey,
  type CookieFormValue,
  type OverrideMap,
} from "./request-editor-model";


type RequestEditorProps = {
  onCancel?: typeof cancelRequestExecution;
  onExecute?: typeof startRequestExecution;
};


export function RequestEditor({
  onCancel = cancelRequestExecution,
  onExecute = startRequestExecution,
}: RequestEditorProps) {
  const { locale, setLocale, t } = useI18n();
  const { density, setDensity, setTheme, theme } = usePreferences();
  const queryClient = useQueryClient();
  const [activeTabId, setActiveTabId] = useState<string | null>(null);
  const [overrides, setOverrides] = useState<OverrideMap>({});
  const [executions, setExecutions] = useState<
    Record<string, ResponseExecutionState>
  >({});
  const [diagnosticsOpen, setDiagnosticsOpen] = useState(false);
  const updateCheckMutation = useMutation({ mutationFn: checkForUpdate });

  const workspaces = useQuery(workspaceQuery);
  const selectedWorkspaceId = workspaces.data?.selectedWorkspaceId;
  const selectedWorkspace =
    workspaces.data?.workspaces.find((workspace) => workspace.isSelected) ?? null;
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
      activeContent ? requestContentQueryKey(activeContent) : null,
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
        throw new Error(t("app.unavailable"));
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
    const name = window.prompt(t("app.folderName"), t("app.newFolder"))?.trim();
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
    const name = window.prompt(t("app.folderName"), folder.name)?.trim();
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
    const tabExecution = executions[tab.draftId] ?? null;
    if (tabExecution && !isTerminalResponseExecution(tabExecution)) {
      const shouldCancel = window.confirm(
        t("app.runningClose"),
      );
      if (!shouldCancel) {
        return;
      }
      await onCancel({ executionId: tabExecution.executionId });
    }
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
          error: t("app.executionFinished"),
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

  async function handleSetBaseDirectory() {
    if (!selectedWorkspaceId) {
      return;
    }
    const baseDirectory = window
      .prompt(t("app.baseDirectory"), selectedWorkspace?.baseDirectory ?? "")
      ?.trim();
    if (baseDirectory === undefined) {
      return;
    }
    await setWorkspaceBaseDirectory(queryClient, {
      workspaceId: selectedWorkspaceId,
      baseDirectory: baseDirectory || null,
    });
  }

  async function handleRelinkBodyFiles() {
    if (!selectedWorkspaceId) {
      return;
    }
    const fromPath = window.prompt(t("app.storedBodyPath"))?.trim();
    if (!fromPath) {
      return;
    }
    const replacementPath = window.prompt(t("app.replacementBodyPath"))?.trim();
    if (!replacementPath) {
      return;
    }
    await relinkBodyFiles(queryClient, {
      workspaceId: selectedWorkspaceId,
      fromPath,
      replacementPath,
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
        <p className="text-sm">{t("app.loading")}</p>
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
            {t("app.unavailable")}
          </h1>
          <button
            className="mt-4 inline-flex items-center gap-2 rounded-md border border-slate-300 bg-white px-3 py-2 text-sm font-medium hover:bg-slate-50 focus-visible:outline focus-visible:outline-2 focus-visible:outline-sky-500"
            onClick={() => {
              void queryClient.invalidateQueries({ queryKey: workspaceQueryKey });
            }}
            type="button"
          >
            <RotateCcw aria-hidden="true" size={16} />
            {t("app.retry")}
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
      <header className="relative flex min-h-12 items-center justify-between border-b border-slate-300 bg-white px-4">
        <h1 className="text-sm font-semibold">Postmite</h1>
        <div className="flex items-center gap-2">
          <label className="sr-only" htmlFor="app-theme">{t("app.theme")}</label>
          <select
            aria-label={t("app.theme")}
            className="h-8 rounded-md border border-slate-300 bg-white px-2 text-sm"
            id="app-theme"
            onChange={(event) => setTheme(event.currentTarget.value as Theme)}
            value={theme}
          >
            <option value="light">{t("app.theme.light")}</option>
            <option value="dark">{t("app.theme.dark")}</option>
            <option value="system">{t("app.theme.system")}</option>
          </select>
          <label className="sr-only" htmlFor="app-density">{t("app.density")}</label>
          <select
            aria-label={t("app.density")}
            className="h-8 rounded-md border border-slate-300 bg-white px-2 text-sm"
            id="app-density"
            onChange={(event) => setDensity(event.currentTarget.value as Density)}
            value={density}
          >
            <option value="comfortable">{t("app.density.comfortable")}</option>
            <option value="compact">{t("app.density.compact")}</option>
          </select>
          <button
            aria-live="polite"
            className="inline-flex h-8 items-center gap-2 rounded-md border border-slate-300 bg-white px-3 text-sm font-medium hover:bg-slate-50 focus-visible:outline focus-visible:outline-2 focus-visible:outline-sky-500 disabled:cursor-not-allowed disabled:opacity-60"
            disabled={updateCheckMutation.isPending}
            onClick={() => updateCheckMutation.mutate()}
            type="button"
          >
            {updateCheckMutation.isPending ? t("app.checkingUpdates") : t("app.checkUpdates")}
          </button>
          <button
            aria-label={t("app.diagnostics")}
            className="inline-flex h-8 w-8 items-center justify-center rounded-md border border-slate-300 bg-white hover:bg-slate-50 focus-visible:outline focus-visible:outline-2 focus-visible:outline-sky-500"
            onClick={() => setDiagnosticsOpen((open) => !open)}
            type="button"
          >
            <Bug aria-hidden="true" size={16} />
          </button>
          <button
            className="inline-flex h-8 items-center gap-2 rounded-md border border-slate-300 bg-white px-3 text-sm font-medium hover:bg-slate-50 focus-visible:outline focus-visible:outline-2 focus-visible:outline-sky-500"
            onClick={() => void handleSetBaseDirectory()}
            type="button"
          >
            <Folder aria-hidden="true" size={16} />
            {t("app.base")}
          </button>
          <button
            className="inline-flex h-8 items-center gap-2 rounded-md border border-slate-300 bg-white px-3 text-sm font-medium hover:bg-slate-50 focus-visible:outline focus-visible:outline-2 focus-visible:outline-sky-500"
            onClick={() => void handleRelinkBodyFiles()}
            type="button"
          >
            <RotateCcw aria-hidden="true" size={16} />
            {t("app.relink")}
          </button>
          <button
            className="inline-flex h-8 items-center gap-2 rounded-md bg-slate-900 px-3 text-sm font-medium text-white hover:bg-slate-700 focus-visible:outline focus-visible:outline-2 focus-visible:outline-sky-500 disabled:cursor-not-allowed disabled:opacity-60"
            disabled={openTabMutation.isPending}
            onClick={() => openTabMutation.mutate()}
            type="button"
          >
            <Plus aria-hidden="true" size={16} />
            {t("app.new")}
          </button>
          <label className="sr-only" htmlFor="app-language">{t("app.language")}</label>
          <select
            aria-label={t("app.language")}
            className="h-8 rounded-md border border-slate-300 bg-white px-2 text-sm focus-visible:outline focus-visible:outline-2 focus-visible:outline-sky-500"
            id="app-language"
            onChange={(event) => setLocale(event.currentTarget.value as AppLocale)}
            value={locale}
          >
            <option value="en">English</option>
            <option value="ja">日本語</option>
          </select>
        </div>
        {diagnosticsOpen ? <DiagnosticsPanel onClose={() => setDiagnosticsOpen(false)} /> : null}
        {updateCheckMutation.isSuccess ? (
          <p className="absolute right-4 top-14 z-20 border border-slate-300 bg-white px-3 py-2 text-sm shadow-lg" role="status">
            {updateCheckMutation.data.updateAvailable
              ? t("app.updateAvailable", { version: updateCheckMutation.data.latestVersion })
              : t("app.upToDate")}
          </p>
        ) : null}
        {updateCheckMutation.isError ? <p className="absolute right-4 top-14 z-20 border border-red-300 bg-white px-3 py-2 text-sm text-red-700 shadow-lg" role="alert">{t("app.updateCheckFailed")}</p> : null}
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
            overrides={overrides}
          />

          {activeDraft && activeContent ? (
            <section
              aria-label="Request editor"
              className="grid min-h-0 flex-1 grid-rows-[auto_minmax(0,1fr)_auto] gap-4 p-4"
            >
              <RequestLine
                content={activeContent}
                executionPhase={activeExecution?.phase ?? "idle"}
                executionRunning={activeExecutionRunning}
                onCancel={() => void handleCancel()}
                onChange={changeActiveDraft}
                onExecute={() => void handleExecute()}
                onSave={() => void handleSave()}
                saving={false}
              />
              <div className="grid min-h-0 gap-4 lg:grid-cols-[minmax(0,1fr)_minmax(360px,0.9fr)]">
                <section className="flex min-h-0 flex-col gap-4">
                  <SecurityPanel
                    content={activeContent}
                    onChange={changeActiveDraft}
                    resolution={resolution.data ?? null}
                  />
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
                <BodyEditor
                  body={activeContent.body}
                  workspaceId={selectedWorkspaceId}
                  onChange={(body) =>
                    changeActiveDraft((content) => ({
                      ...content,
                      body,
                    }))
                  }
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
