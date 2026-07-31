import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Plus, RotateCcw } from "lucide-react";
import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";

import { AppHeader } from "../../app/AppHeader";
import { Button } from "../../components/ui/button";
import {
  closeRequestTab,
  createCollectionFolder,
  createEnvironment,
  clearCookies,
  cookieJarQuery,
  deleteCollectionFolder,
  deleteCookie,
  deleteEnvironment,
  deleteSavedRequest,
  duplicateCollectionFolder,
  duplicateSavedRequest,
  executionHistoryQuery,
  executionHistoryQueryKey,
  generateCurl,
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
  updateEnvironment,
} from "../../shared/api/requests";
import {
  applyResponseExecutionEvents,
  cancelRequestExecution,
  createExecutionId,
  createQueuedResponseExecutionState,
  isTerminalResponseExecution,
  listenToRequestExecutionEvents,
  recordFrontendExecutionTrace,
  reduceResponseExecutionStates,
  startRequestExecution,
} from "../../shared/api/execution";
import { writeClipboardText } from "../../shared/api/clipboard";
import {
  createWorkspace,
  deleteWorkspace,
  renameWorkspace,
  switchWorkspace,
  workspaceQuery,
  workspaceQueryKey,
  setWorkspaceBaseDirectory,
} from "../../shared/api/workspaces";
import { checkForUpdate } from "../../shared/api/update";
import type {
  ExecutionEventDto,
  ResponseExecutionState,
} from "../../shared/api/execution";
import type {
  WorkspaceCookieDto,
  CollectionFolderDto,
  ExecutionRecordDto,
  RequestContentDto,
  RequestDraftDto,
  ResolvedRequestContentDto,
  SavedRequestDto,
  RequestTabDto,
  EnvironmentVariableDraftDto,
} from "../../shared/api/generated/ipc";
import { RequestLine } from "./controls/RequestLine";
import {
  CurlCopyControl,
  type CurlCopyFeedback,
} from "./controls/CurlCopyControl";
import { TabStrip } from "./controls/TabStrip";
import { useMediaQuery } from "./hooks/useMediaQuery";
import { RequestEditorPanels } from "./layout/RequestEditorPanels";
import { RequestWorkspaceShell } from "./layout/RequestWorkspaceShell";
import { CollectionsSidebar } from "./panels/CollectionsSidebar";
import { WorkspaceManagerDialog } from "../workspace/WorkspaceManagerDialog";
import { EnvironmentManagerDialog } from "../environment/EnvironmentManagerDialog";
import { DiagnosticsPanel } from "./panels/DiagnosticsPanel";
import { useI18n } from "../../app/i18n";
import { usePreferences } from "../../app/preferences";
import {
  emptyRequestContent,
  isDraftDirty,
  omitKey,
  requestContentQueryKey,
  type CookieFormValue,
  type OverrideMap,
} from "./models/request-editor-model";


type RequestEditorProps = {
  onCancel?: typeof cancelRequestExecution;
  onExecute?: typeof startRequestExecution;
};

type CurlCopyContext = {
  content: RequestContentDto;
  environmentId: string | null;
  fingerprint: string;
  resolution: ResolvedRequestContentDto;
  resolutionQueryKey: readonly unknown[];
  version: number;
  workspaceId: string;
};

type PendingCurlConfirmation = {
  context: CurlCopyContext;
  redactedCommand: string;
};

function isTraceableExecutionEvent(event: ExecutionEventDto) {
  return (
    event.kind.type === "STARTED" ||
    event.kind.type === "RESPONSE_HEADERS" ||
    event.kind.type === "COMPLETED" ||
    event.kind.type === "FAILED" ||
    event.kind.type === "CANCELLED"
  );
}

export function RequestEditor({
  onCancel = cancelRequestExecution,
  onExecute = startRequestExecution,
}: RequestEditorProps) {
  const isEditorResizableLayout = useMediaQuery("(min-width: 1024px)", true);
  const { locale, setLocale, t } = useI18n();
  const {
    density,
    requestResponseSplit,
    setDensity,
    setRequestResponseSplit,
    setTheme,
    theme,
  } = usePreferences();
  const queryClient = useQueryClient();
  const [activeTabId, setActiveTabId] = useState<string | null>(null);
  const [overrides, setOverrides] = useState<OverrideMap>({});
  const latestContentRef = useRef<OverrideMap>({});
  const [executions, setExecutions] = useState<
    Record<string, ResponseExecutionState>
  >({});
  const executionsRef = useRef<Record<string, ResponseExecutionState>>({});
  const pendingExecutionEventsRef = useRef<Map<string, ExecutionEventDto[]>>(new Map());
  const [executionListenerState, setExecutionListenerState] = useState<
    "pending" | "ready" | "failed"
  >("pending");
  const [diagnosticsOpen, setDiagnosticsOpen] = useState(false);
  const [workspaceManagerOpen, setWorkspaceManagerOpen] = useState(false);
  const [environmentManagerOpen, setEnvironmentManagerOpen] = useState(false);
  const [curlCopyFeedback, setCurlCopyFeedback] =
    useState<CurlCopyFeedback>(null);
  const [curlCopyPending, setCurlCopyPending] = useState(false);
  const [pendingCurlConfirmation, setPendingCurlConfirmation] =
    useState<PendingCurlConfirmation | null>(null);
  const currentCurlFingerprintRef = useRef<string | null>(null);
  const curlContextVersionRef = useRef(0);
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
    activeDraft
      ? latestContentRef.current[activeDraft.id] ??
        overrides[activeDraft.id] ??
        activeDraft.content
      : null;
  const selectedEnvironment =
    snapshot?.environments.find((environment) => environment.isSelected) ?? null;
  const activeExecution = activeDraft ? executions[activeDraft.id] ?? null : null;
  const activeExecutionRunning =
    activeExecution !== null && !isTerminalResponseExecution(activeExecution);
  const activeContentKey = activeContent
    ? requestContentQueryKey(activeContent)
    : null;
  const resolutionQueryKey = [
    "requestResolution",
    selectedWorkspaceId,
    activeDraft?.id ?? null,
    selectedEnvironment?.id ?? null,
    activeContentKey,
  ] as const;
  const curlContextFingerprint =
    selectedWorkspaceId && activeDraft && activeContentKey
      ? JSON.stringify([
          selectedWorkspaceId,
          activeDraft.id,
          selectedEnvironment?.id ?? null,
          activeContentKey,
        ])
      : null;
  useLayoutEffect(() => {
    currentCurlFingerprintRef.current = curlContextFingerprint;
  }, [curlContextFingerprint]);
  const resolution = useQuery({
    queryKey: resolutionQueryKey,
    queryFn: () =>
      resolveRequestContent({
        workspaceId: selectedWorkspaceId ?? "",
        content: activeContent ?? emptyRequestContent(),
      }),
    enabled: Boolean(selectedWorkspaceId && activeDraft && activeContent),
  });

  function currentCurlContext(): CurlCopyContext | null {
    if (
      !activeContent ||
      !selectedWorkspaceId ||
      !curlContextFingerprint ||
      !resolution.data ||
      !resolution.isSuccess ||
      resolution.isFetching
    ) {
      return null;
    }
    const queryState =
      queryClient.getQueryState<ResolvedRequestContentDto>(resolutionQueryKey);
    if (
      queryState?.status !== "success" ||
      queryState.fetchStatus !== "idle" ||
      queryState.data !== resolution.data
    ) {
      return null;
    }
    return {
      content: activeContent,
      environmentId: selectedEnvironment?.id ?? null,
      fingerprint: curlContextFingerprint,
      resolution: resolution.data,
      resolutionQueryKey,
      version: curlContextVersionRef.current,
      workspaceId: selectedWorkspaceId,
    };
  }

  function isCurlContextCurrent(context: CurlCopyContext) {
    if (
      currentCurlFingerprintRef.current !== context.fingerprint ||
      curlContextVersionRef.current !== context.version
    ) {
      return false;
    }
    const queryState =
      queryClient.getQueryState<ResolvedRequestContentDto>(
        context.resolutionQueryKey,
      );
    return (
      queryState?.status === "success" &&
      queryState.fetchStatus === "idle" &&
      queryState.data === context.resolution
    );
  }

  function invalidateCurlCopyContext() {
    curlContextVersionRef.current += 1;
    setPendingCurlConfirmation(null);
  }

  async function copyCurlCommand(command: string, context: CurlCopyContext) {
    if (!isCurlContextCurrent(context)) {
      setCurlCopyFeedback("stale");
      return;
    }
    try {
      await writeClipboardText(command);
      setCurlCopyFeedback("copied");
    } catch {
      setCurlCopyFeedback("failed");
    }
  }

  async function handleCopyCurl() {
    const context = currentCurlContext();
    if (!context) {
      setCurlCopyFeedback("stale");
      return;
    }
    setCurlCopyFeedback(null);
    setCurlCopyPending(true);
    try {
      const generated = await generateCurl({
        workspaceId: context.workspaceId,
        environmentId: context.environmentId,
        content: context.content,
        resolved: context.resolution,
        includeSecrets: false,
      });
      if (!isCurlContextCurrent(context)) {
        setCurlCopyFeedback("stale");
        return;
      }
      if (generated.redactedSecretCount > 0) {
        setPendingCurlConfirmation({
          context,
          redactedCommand: generated.command,
        });
        return;
      }
      await copyCurlCommand(generated.command, context);
    } catch {
      setCurlCopyFeedback("failed");
    } finally {
      setCurlCopyPending(false);
    }
  }

  async function handleCopyRedactedCurl() {
    const pending = pendingCurlConfirmation;
    setPendingCurlConfirmation(null);
    if (!pending) {
      return;
    }
    await copyCurlCommand(pending.redactedCommand, pending.context);
  }

  async function handleCopyCurlWithSecrets() {
    const pending = pendingCurlConfirmation;
    setPendingCurlConfirmation(null);
    if (!pending || !isCurlContextCurrent(pending.context)) {
      setCurlCopyFeedback("stale");
      return;
    }
    setCurlCopyPending(true);
    try {
      const generated = await generateCurl({
        workspaceId: pending.context.workspaceId,
        environmentId: pending.context.environmentId,
        content: pending.context.content,
        resolved: pending.context.resolution,
        includeSecrets: true,
      });
      if (!isCurlContextCurrent(pending.context)) {
        setCurlCopyFeedback("stale");
        return;
      }
      await copyCurlCommand(generated.command, pending.context);
    } catch {
      setCurlCopyFeedback("failed");
    } finally {
      setCurlCopyPending(false);
    }
  }

  function updateExecutions(
    updater: (
      current: Record<string, ResponseExecutionState>,
    ) => Record<string, ResponseExecutionState>,
  ) {
    const next = updater(executionsRef.current);
    executionsRef.current = next;
    setExecutions(next);
    return next;
  }

  useEffect(() => {
    let unlisten: (() => void) | null = null;
    let disposed = false;
    setExecutionListenerState("pending");

    void listenToRequestExecutionEvents((event) => {
      const current = executionsRef.current;
      const matched = Object.values(current).some(
        (execution) => execution.executionId === event.executionId,
      );
      const next = reduceResponseExecutionStates(current, event, Date.now());
      executionsRef.current = next;
      setExecutions(next);
      let traceStage: "EVENT_APPLIED" | "EVENT_BUFFERED" | "EVENT_IGNORED";
      if (next !== current) {
        traceStage = "EVENT_APPLIED";
      } else if (matched) {
        traceStage = "EVENT_IGNORED";
      } else {
        const pending = pendingExecutionEventsRef.current.get(event.executionId) ?? [];
        pendingExecutionEventsRef.current.set(event.executionId, [...pending, event]);
        traceStage = "EVENT_BUFFERED";
      }
      if (isTraceableExecutionEvent(event)) {
        void recordFrontendExecutionTrace(
          event.executionId,
          traceStage,
          event.sequence,
        );
      }
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
      setExecutionListenerState("ready");
    }).catch(() => {
      if (!disposed) {
        setExecutionListenerState("failed");
      }
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
      invalidateCurlCopyContext();
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
    invalidateCurlCopyContext();
    await selectEnvironment(queryClient, {
      workspaceId: selectedWorkspaceId,
      environmentId,
    });
  }

  async function handleSwitchWorkspace(workspaceId: string) {
    if (workspaceId === selectedWorkspaceId) return;
    invalidateCurlCopyContext();
    setActiveTabId(null);
    await switchWorkspace(queryClient, { workspaceId });
  }

  async function handleCreateWorkspace(name: string) {
    invalidateCurlCopyContext();
    setActiveTabId(null);
    await createWorkspace(queryClient, { name });
    setWorkspaceManagerOpen(false);
  }

  async function handleRenameWorkspace(workspaceId: string, name: string) {
    await renameWorkspace(queryClient, { workspaceId, name });
  }

  async function handleDeleteWorkspace(workspaceId: string) {
    invalidateCurlCopyContext();
    setActiveTabId(null);
    await deleteWorkspace(queryClient, { workspaceId });
    setWorkspaceManagerOpen(false);
  }

  async function handleCreateEnvironment(name: string) {
    invalidateCurlCopyContext();
    return createEnvironment(queryClient, { workspaceId: selectedWorkspaceId!, name });
  }

  async function handleUpdateEnvironment(
    environmentId: string,
    name: string,
    variables: EnvironmentVariableDraftDto[],
  ) {
    invalidateCurlCopyContext();
    return updateEnvironment(queryClient, {
      workspaceId: selectedWorkspaceId!,
      environmentId,
      name,
      variables,
    });
  }

  async function handleDeleteEnvironment(environmentId: string) {
    invalidateCurlCopyContext();
    return deleteEnvironment(queryClient, {
      workspaceId: selectedWorkspaceId!,
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
    invalidateCurlCopyContext();
    const nextSnapshot = await openSavedRequestTab(queryClient, {
      workspaceId: request.workspaceId,
      savedRequestId: request.id,
    });
    setActiveTabId(
      nextSnapshot.tabs.find((tab) => tab.savedRequestId === request.id)?.id ??
        activeTabId,
    );
  }

  async function handleMoveSavedRequest(
    request: SavedRequestDto,
    location: { collectionId: string | null; position: number },
  ) {
    await moveSavedRequest(queryClient, {
      workspaceId: request.workspaceId,
      savedRequestId: request.id,
      location,
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
    if (!activeDraft) {
      return;
    }
    invalidateCurlCopyContext();

    const base =
      latestContentRef.current[activeDraft.id] ??
      overrides[activeDraft.id] ??
      activeDraft.content;
    const nextContent = updater(base);
    latestContentRef.current = {
      ...latestContentRef.current,
      [activeDraft.id]: nextContent,
    };
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
    latestContentRef.current = omitKey(latestContentRef.current, activeDraft.id);
    setActiveTabId(
      nextSnapshot.tabs.find((tab) => tab.draftId === activeDraft.id)?.id ?? activeTabId,
    );
  }

  async function handleClose(tab: RequestTabDto, decision: "SAVE" | "DISCARD") {
    invalidateCurlCopyContext();
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
    latestContentRef.current = omitKey(latestContentRef.current, tab.draftId);
    setActiveTabId(
      nextSnapshot.tabs.find((item) => item.isActive)?.id ??
        nextSnapshot.tabs[0]?.id ??
        null,
    );
  }

  async function handleExecute() {
    if (
      !activeDraft ||
      !activeContent ||
      executionListenerState !== "ready"
    ) {
      return;
    }

    await persistDraft(activeDraft, activeContent);
    const executionId = createExecutionId();
    updateExecutions((current) => ({
      ...current,
      [activeDraft.id]: createQueuedResponseExecutionState({
        draftId: activeDraft.id,
        executionId,
        nowMs: Date.now(),
      }),
    }));
    let result;
    try {
      result = await onExecute({
        workspaceId: activeDraft.workspaceId,
        draftId: activeDraft.id,
        executionId,
        content: activeContent,
      });
    } catch (error) {
      const message = error instanceof Error ? error.message : t("app.unavailable");
      updateExecutions((current) => {
        const currentExecution = current[activeDraft.id];
        if (currentExecution?.executionId !== executionId) {
          return current;
        }
        return {
          ...current,
          [activeDraft.id]: {
            ...currentExecution,
            phase: "failed",
            completedAtMs: Date.now(),
            error: message,
          },
        };
      });
      return;
    }
    const reconciledExecutions = updateExecutions((current) => {
      const currentExecution = current[activeDraft.id];
      const base =
        currentExecution?.executionId === result.executionId
          ? currentExecution
          : createQueuedResponseExecutionState({
              draftId: activeDraft.id,
              executionId: result.executionId,
              nowMs: Date.now(),
            });
      const pending = pendingExecutionEventsRef.current.get(result.executionId) ?? [];
      pendingExecutionEventsRef.current.delete(result.executionId);
      const initialEvents = [...result.initialEvents, ...pending].sort((left, right) =>
        Number(left.sequence - right.sequence),
      );
      return {
        ...current,
        [activeDraft.id]: applyResponseExecutionEvents(
          base,
          initialEvents,
          Date.now(),
        ),
      };
    });
    const reconciled = reconciledExecutions[activeDraft.id];
    const reconciledTerminal =
      reconciled?.executionId === result.executionId &&
      isTerminalResponseExecution(reconciled);
    void recordFrontendExecutionTrace(
      result.executionId,
      reconciledTerminal
        ? "START_RECONCILED_TERMINAL"
        : "START_RECONCILED_PENDING",
    );
  }

  async function handleCancel() {
    if (!activeExecution || isTerminalResponseExecution(activeExecution)) {
      return;
    }

    const result = await onCancel({ executionId: activeExecution.executionId });
    if (!result.cancelled) {
      updateExecutions((current) => ({
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
    invalidateCurlCopyContext();
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
      <main className="flex h-full items-center justify-center overflow-hidden bg-slate-100 text-slate-950">
        <p className="text-sm">{t("app.loading")}</p>
      </main>
    );
  }

  if (workspaces.isError || requestWorkspace.isError || !selectedWorkspaceId) {
    return (
      <main className="flex h-full items-center justify-center overflow-hidden bg-slate-100 p-6 text-slate-950">
        <section
          aria-labelledby="request-editor-error"
          className="w-full max-w-xl rounded-md border border-red-300 bg-white p-5"
        >
          <h1 id="request-editor-error" className="text-base font-semibold">
            {t("app.unavailable")}
          </h1>
          <Button
            className="mt-4"
            onClick={() => {
              void queryClient.invalidateQueries({ queryKey: workspaceQueryKey });
            }}
            type="button"
            variant="outline"
          >
            <RotateCcw aria-hidden="true" size={16} />
            {t("app.retry")}
          </Button>
        </section>
      </main>
    );
  }

  const sidebar = (
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
      onMoveRequest={(request, location) =>
        void handleMoveSavedRequest(request, location)
      }
      onOpenRequest={(request) => void handleOpenSavedRequest(request)}
      onManageEnvironments={() => setEnvironmentManagerOpen(true)}
      onRenameFolder={(folder) => void handleRenameCollectionFolder(folder)}
      onSelectEnvironment={(environmentId) =>
        void handleSelectEnvironment(environmentId)
      }
      requests={snapshot?.savedRequests ?? []}
    />
  );

  const editorPane = (
    <div className="flex h-full min-h-0 flex-col bg-background">
      <TabStrip
        activeTabId={activeTab?.id ?? null}
        drafts={snapshot?.drafts ?? []}
        onActivate={(tabId) => {
          invalidateCurlCopyContext();
          setActiveTabId(tabId);
        }}
        onClose={(tab) => void handleClose(tab, isDraftDirty(tab.draftId, snapshot?.drafts ?? [], overrides) ? "SAVE" : "DISCARD")}
        overrides={overrides}
        tabs={tabs}
      />

      {activeDraft && activeContent ? (
        <section
          aria-label="Request editor"
          className="flex min-h-0 flex-1 flex-col gap-4 overflow-hidden p-4"
        >
          <RequestLine
            content={activeContent}
            executionPhase={activeExecution?.phase ?? "idle"}
            executionReady={executionListenerState === "ready"}
            executionRunning={activeExecutionRunning}
            onCancel={() => void handleCancel()}
            onChange={changeActiveDraft}
            onExecute={() => void handleExecute()}
            onSave={() => void handleSave()}
            saving={false}
          />
          {executionListenerState === "failed" ? (
            <p className="text-sm text-red-700" role="alert">
              {t("app.executionEventsUnavailable")}
            </p>
          ) : null}
          <RequestEditorPanels
            content={activeContent}
            cookies={cookieJar.data?.cookies ?? []}
            cookiesLoading={cookieJar.isFetching}
            curlAction={
              <CurlCopyControl
                confirmationOpen={Boolean(
                  pendingCurlConfirmation &&
                    isCurlContextCurrent(pendingCurlConfirmation.context),
                )}
                disabled={curlCopyPending || currentCurlContext() === null}
                feedback={curlCopyFeedback}
                onCancelConfirmation={() => setPendingCurlConfirmation(null)}
                onCopy={() => void handleCopyCurl()}
                onCopyRedacted={() => void handleCopyRedactedCurl()}
                onIncludeSecrets={() => void handleCopyCurlWithSecrets()}
                pending={curlCopyPending}
              />
            }
            execution={activeExecution}
            history={executionHistory.data ?? null}
            historyLoading={executionHistory.isFetching}
            onChange={changeActiveDraft}
            onClearCookies={() => void handleClearCookies()}
            onDeleteCookie={(cookie) => void handleDeleteCookie(cookie)}
            onOpenHistoryRecord={(record) => void handleOpenHistoryRecord(record)}
            onRevealCookie={(cookie) => handleRevealCookie(cookie)}
            onSaveCookie={(input) => void handleUpsertCookie(input)}
            onToggleHistoryDisabled={(disabled) =>
              void handleToggleHistoryDisabled(disabled)
            }
            onToggleHistoryPinned={(record) => void handleToggleHistoryPinned(record)}
            requestResponseSplit={requestResponseSplit}
            resizable={isEditorResizableLayout}
            resolution={resolution.data ?? null}
            resolving={resolution.isFetching}
            workspaceId={selectedWorkspaceId}
          />
        </section>
      ) : (
        <section className="flex flex-1 items-center justify-center p-6">
          <Button onClick={() => openTabMutation.mutate()} type="button">
            <Plus aria-hidden="true" size={16} />
            New Request
          </Button>
        </section>
      )}
    </div>
  );

  return (
    <main
      className="flex h-full flex-col overflow-hidden bg-muted text-foreground"
      onKeyDown={handleEditorKeyDown}
    >
      <AppHeader
        checkingUpdates={updateCheckMutation.isPending}
        density={density}
        diagnosticsOpen={diagnosticsOpen}
        locale={locale}
        newRequestPending={openTabMutation.isPending}
        onCheckUpdates={() => updateCheckMutation.mutate()}
        onNewRequest={() => openTabMutation.mutate()}
        onManageWorkspaces={() => setWorkspaceManagerOpen(true)}
        onRelinkBodyFiles={() => void handleRelinkBodyFiles()}
        onSetBaseDirectory={() => void handleSetBaseDirectory()}
        onToggleDiagnostics={() => setDiagnosticsOpen((open) => !open)}
        onSelectWorkspace={(workspaceId) => void handleSwitchWorkspace(workspaceId)}
        requestResponseSplit={requestResponseSplit}
        setDensity={setDensity}
        setLocale={setLocale}
        setRequestResponseSplit={setRequestResponseSplit}
        setTheme={setTheme}
        theme={theme}
        updateError={updateCheckMutation.isError}
        updateResult={updateCheckMutation.isSuccess ? updateCheckMutation.data : null}
        selectedWorkspaceId={selectedWorkspaceId}
        workspaces={workspaces.data?.workspaces ?? []}
      />
      <div className="relative">
        {diagnosticsOpen ? <DiagnosticsPanel onClose={() => setDiagnosticsOpen(false)} /> : null}
      </div>

      <RequestWorkspaceShell editorPane={editorPane} sidebar={sidebar} />
      <WorkspaceManagerDialog
        onCreate={handleCreateWorkspace}
        onDelete={handleDeleteWorkspace}
        onOpenChange={setWorkspaceManagerOpen}
        onRename={handleRenameWorkspace}
        onSelect={handleSwitchWorkspace}
        open={workspaceManagerOpen}
        selectedWorkspaceId={selectedWorkspaceId}
        workspaces={workspaces.data?.workspaces ?? []}
      />
      <EnvironmentManagerDialog
        environments={snapshot?.environments ?? []}
        environmentVariables={snapshot?.environmentVariables ?? []}
        onCreate={handleCreateEnvironment}
        onDelete={handleDeleteEnvironment}
        onOpenChange={setEnvironmentManagerOpen}
        onSave={handleUpdateEnvironment}
        onSelect={handleSelectEnvironment}
        open={environmentManagerOpen}
      />
    </main>
  );
}
