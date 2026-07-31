import { invoke } from "@tauri-apps/api/core";

import type {
  CloseRequestTabInput,
  CancelOAuthAuthorizationInput,
  CancelRequestExecutionInput,
  CookieIdInput,
  CollectionFolderIdInput,
  CreateSavedRequestInput,
  CreateCollectionFolderInput,
  CreateEnvironmentInput,
  CreateWorkspaceInput,
  CurlGenerateInput,
  CurlImportInput,
  DiagnosticBundleExportInput,
  DiagnosticDebugLoggingInput,
  FrontendExecutionTraceInput,
  DescribeBodyFileInput,
  ExecutionRecordIdInput,
  EnvironmentIdInput,
  IpcCommandContracts,
  IpcError,
  MoveCollectionFolderInput,
  MoveSavedRequestInput,
  NativeBackupExportInput,
  NativeBackupRestoreInput,
  NativeBackupRestorePreviewInput,
  OpenSavedRequestTabInput,
  PostmanExportInput,
  PostmanImportInput,
  PostmanReimportInput,
  RecoverableDatabaseExportInput,
  RequestDraftIdInput,
  RelinkBodyFilesInput,
  ResolveRequestContentInput,
  RenameWorkspaceInput,
  RenameCollectionFolderInput,
  SavedRequestIdInput,
  SaveResponseFileInput,
  SelectEnvironmentInput,
  SetExecutionHistoryDisabledInput,
  SetExecutionRecordPinnedInput,
  SetWorkspaceBaseDirectoryInput,
  StartOAuthAuthorizationInput,
  StartRequestExecutionInput,
  UpdateRequestDraftInput,
  UpdateEnvironmentInput,
  UpsertCookieInput,
  WorkspaceIdInput,
} from "./generated/ipc";

export class IpcCommandError extends Error implements IpcError {
  readonly code: IpcError["code"];
  readonly details: string | null;
  readonly retryable: boolean;

  constructor(error: IpcError) {
    super(error.message);
    this.name = "IpcCommandError";
    this.code = error.code;
    this.details = error.details;
    this.retryable = error.retryable;
  }
}

export async function invokeCommand<Command extends IpcCommandName>(
  command: Command,
  input: IpcCommandContracts[Command]["input"],
): Promise<IpcCommandContracts[Command]["output"]> {
  try {
    if (input === undefined) {
      return await invoke<IpcCommandContracts[Command]["output"]>(command);
    }

    return await invoke<IpcCommandContracts[Command]["output"]>(command, {
      input,
    });
  } catch (error) {
    throw normalizeIpcError(error);
  }
}

export const workspaceIpc = {
  listWorkspaces: () => invokeCommand("list_workspaces", undefined),
  createWorkspace: (input: CreateWorkspaceInput) =>
    invokeCommand("create_workspace", input),
  renameWorkspace: (input: RenameWorkspaceInput) =>
    invokeCommand("rename_workspace", input),
  setWorkspaceBaseDirectory: (input: SetWorkspaceBaseDirectoryInput) =>
    invokeCommand("set_workspace_base_directory", input),
  switchWorkspace: (input: WorkspaceIdInput) =>
    invokeCommand("switch_workspace", input),
  deleteWorkspace: (input: WorkspaceIdInput) =>
    invokeCommand("delete_workspace", input),
};

export const requestIpc = {
  listRequestWorkspace: (input: WorkspaceIdInput) =>
    invokeCommand("list_request_workspace", input),
  openUnsavedRequestTab: (input: WorkspaceIdInput) =>
    invokeCommand("open_unsaved_request_tab", input),
  createSavedRequest: (input: CreateSavedRequestInput) =>
    invokeCommand("create_saved_request", input),
  createCollectionFolder: (input: CreateCollectionFolderInput) =>
    invokeCommand("create_collection_folder", input),
  selectEnvironment: (input: SelectEnvironmentInput) =>
    invokeCommand("select_environment", input),
  createEnvironment: (input: CreateEnvironmentInput) =>
    invokeCommand("create_environment", input),
  updateEnvironment: (input: UpdateEnvironmentInput) =>
    invokeCommand("update_environment", input),
  deleteEnvironment: (input: EnvironmentIdInput) =>
    invokeCommand("delete_environment", input),
  resolveRequestContent: (input: ResolveRequestContentInput) =>
    invokeCommand("resolve_request_content", input),
  renameCollectionFolder: (input: RenameCollectionFolderInput) =>
    invokeCommand("rename_collection_folder", input),
  moveCollectionFolder: (input: MoveCollectionFolderInput) =>
    invokeCommand("move_collection_folder", input),
  duplicateCollectionFolder: (input: CollectionFolderIdInput) =>
    invokeCommand("duplicate_collection_folder", input),
  deleteCollectionFolder: (input: CollectionFolderIdInput) =>
    invokeCommand("delete_collection_folder", input),
  moveSavedRequest: (input: MoveSavedRequestInput) =>
    invokeCommand("move_saved_request", input),
  duplicateSavedRequest: (input: SavedRequestIdInput) =>
    invokeCommand("duplicate_saved_request", input),
  deleteSavedRequest: (input: SavedRequestIdInput) =>
    invokeCommand("delete_saved_request", input),
  openSavedRequestTab: (input: OpenSavedRequestTabInput) =>
    invokeCommand("open_saved_request_tab", input),
  updateRequestDraft: (input: UpdateRequestDraftInput) =>
    invokeCommand("update_request_draft", input),
  flushRequestDrafts: () => invokeCommand("flush_request_drafts", undefined),
  saveRequestDraft: (input: RequestDraftIdInput) =>
    invokeCommand("save_request_draft", input),
  closeRequestTab: (input: CloseRequestTabInput) =>
    invokeCommand("close_request_tab", input),
  listExecutionHistory: (input: WorkspaceIdInput) =>
    invokeCommand("list_execution_history", input),
  setExecutionHistoryDisabled: (input: SetExecutionHistoryDisabledInput) =>
    invokeCommand("set_execution_history_disabled", input),
  setExecutionRecordPinned: (input: SetExecutionRecordPinnedInput) =>
    invokeCommand("set_execution_record_pinned", input),
  openExecutionRecordAsDraft: (input: ExecutionRecordIdInput) =>
    invokeCommand("open_execution_record_as_draft", input),
  listCookies: (input: WorkspaceIdInput) => invokeCommand("list_cookies", input),
  upsertCookie: (input: UpsertCookieInput) =>
    invokeCommand("upsert_cookie", input),
  deleteCookie: (input: CookieIdInput) => invokeCommand("delete_cookie", input),
  clearCookies: (input: WorkspaceIdInput) => invokeCommand("clear_cookies", input),
  revealCookieValue: (input: CookieIdInput) =>
    invokeCommand("reveal_cookie_value", input),
  describeBodyFile: (input: DescribeBodyFileInput) =>
    invokeCommand("describe_body_file", input),
  relinkBodyFiles: (input: RelinkBodyFilesInput) =>
    invokeCommand("relink_body_files", input),
  previewPostmanImport: (input: PostmanImportInput) =>
    invokeCommand("preview_postman_import", input),
  importPostman: (input: PostmanImportInput) =>
    invokeCommand("import_postman", input),
  exportPostman: (input: PostmanExportInput) =>
    invokeCommand("export_postman", input),
  previewPostmanReimport: (input: PostmanImportInput) =>
    invokeCommand("preview_postman_reimport", input),
  reimportPostman: (input: PostmanReimportInput) =>
    invokeCommand("reimport_postman", input),
  exportNativeBackup: (input: NativeBackupExportInput) =>
    invokeCommand("export_native_backup", input),
  previewNativeBackupRestore: (input: NativeBackupRestorePreviewInput) =>
    invokeCommand("preview_native_backup_restore", input),
  restoreNativeBackup: (input: NativeBackupRestoreInput) =>
    invokeCommand("restore_native_backup", input),
  getDatabaseRecoveryState: () =>
    invokeCommand("get_database_recovery_state", undefined),
  exportRecoverableDatabase: (input: RecoverableDatabaseExportInput) =>
    invokeCommand("export_recoverable_database", input),
  getDiagnosticBundlePreview: () =>
    invokeCommand("get_diagnostic_bundle_preview", undefined),
  setDiagnosticDebugLogging: (input: DiagnosticDebugLoggingInput) =>
    invokeCommand("set_diagnostic_debug_logging", input),
  recordFrontendExecutionTrace: (input: FrontendExecutionTraceInput) =>
    invokeCommand("record_frontend_execution_trace", input),
  exportDiagnosticBundle: (input: DiagnosticBundleExportInput) =>
    invokeCommand("export_diagnostic_bundle", input),
  checkForUpdate: () => invokeCommand("check_for_update", undefined),
  previewCurlImport: (input: CurlImportInput) =>
    invokeCommand("preview_curl_import", input),
  importCurlAsDraft: (input: CurlImportInput) =>
    invokeCommand("import_curl_as_draft", input),
  generateCurl: (input: CurlGenerateInput) => invokeCommand("generate_curl", input),
  startRequestExecution: (input: StartRequestExecutionInput) =>
    invokeCommand("start_request_execution", input),
  cancelRequestExecution: (input: CancelRequestExecutionInput) =>
    invokeCommand("cancel_request_execution", input),
  saveResponseFile: (input: SaveResponseFileInput) =>
    invokeCommand("save_response_file", input),
  startOAuthAuthorization: (input: StartOAuthAuthorizationInput) =>
    invokeCommand("start_oauth_authorization", input),
  cancelOAuthAuthorization: (input: CancelOAuthAuthorizationInput) =>
    invokeCommand("cancel_oauth_authorization", input),
};

type IpcCommandName = keyof IpcCommandContracts;

function normalizeIpcError(error: unknown) {
  if (isIpcError(error)) {
    return new IpcCommandError(error);
  }

  return new IpcCommandError({
    code: "STATE_UNAVAILABLE",
    message: "IPC command failed.",
    details: null,
    retryable: true,
  });
}

function isIpcError(error: unknown): error is IpcError {
  return (
    typeof error === "object" &&
    error !== null &&
    "code" in error &&
    "message" in error &&
    "retryable" in error &&
    typeof error.code === "string" &&
    typeof error.message === "string" &&
    typeof error.retryable === "boolean" &&
    (!("details" in error) ||
      typeof error.details === "string" ||
      error.details === null)
  );
}
