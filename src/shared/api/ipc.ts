import { invoke } from "@tauri-apps/api/core";

import type {
  CloseRequestTabInput,
  CancelRequestExecutionInput,
  CollectionFolderIdInput,
  CreateSavedRequestInput,
  CreateCollectionFolderInput,
  CreateWorkspaceInput,
  IpcCommandContracts,
  IpcError,
  MoveCollectionFolderInput,
  MoveSavedRequestInput,
  OpenSavedRequestTabInput,
  RequestDraftIdInput,
  RenameWorkspaceInput,
  RenameCollectionFolderInput,
  SavedRequestIdInput,
  StartRequestExecutionInput,
  UpdateRequestDraftInput,
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
  startRequestExecution: (input: StartRequestExecutionInput) =>
    invokeCommand("start_request_execution", input),
  cancelRequestExecution: (input: CancelRequestExecutionInput) =>
    invokeCommand("cancel_request_execution", input),
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
