import { invoke } from "@tauri-apps/api/core";

import type {
  CreateWorkspaceInput,
  IpcError,
  RenameWorkspaceInput,
  WorkspaceCommandContracts,
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

export async function invokeCommand<Command extends WorkspaceCommandName>(
  command: Command,
  input: WorkspaceCommandContracts[Command]["input"],
): Promise<WorkspaceCommandContracts[Command]["output"]> {
  try {
    if (input === undefined) {
      return await invoke<WorkspaceCommandContracts[Command]["output"]>(command);
    }

    return await invoke<WorkspaceCommandContracts[Command]["output"]>(command, {
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

type WorkspaceCommandName = keyof WorkspaceCommandContracts;

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
