import { beforeEach, describe, expect, it, vi } from "vitest";

const invokeMock = vi.hoisted(() => vi.fn());

vi.mock("@tauri-apps/api/core", () => ({
  invoke: invokeMock,
}));

import { invokeCommand, workspaceIpc } from "./ipc";

describe("typed IPC adapter", () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  it("invokes commands with typed input under the Tauri input argument", async () => {
    invokeMock.mockResolvedValue({
      selectedWorkspaceId: "workspace-1",
      workspaces: [],
    });

    await workspaceIpc.createWorkspace({ name: "Client" });

    expect(invokeMock).toHaveBeenCalledWith("create_workspace", {
      input: { name: "Client" },
    });
  });

  it("invokes commands without input when the contract input is undefined", async () => {
    invokeMock.mockResolvedValue({
      selectedWorkspaceId: "workspace-1",
      workspaces: [],
    });

    await invokeCommand("list_workspaces", undefined);

    expect(invokeMock).toHaveBeenCalledWith("list_workspaces");
  });

  it("wraps safe Rust IPC errors without exposing unknown thrown values", async () => {
    invokeMock.mockRejectedValue({
      code: "PERSISTENCE_UNAVAILABLE",
      message: "Workspace persistence is unavailable.",
      details: null,
      retryable: true,
    });

    await expect(workspaceIpc.listWorkspaces()).rejects.toMatchObject({
      name: "IpcCommandError",
      code: "PERSISTENCE_UNAVAILABLE",
      details: null,
      retryable: true,
    });
  });

  it("normalizes unexpected errors to a retryable state error", async () => {
    invokeMock.mockRejectedValue("sentinel transport failure");

    await expect(workspaceIpc.listWorkspaces()).rejects.toEqual(
      expect.objectContaining({
        code: "STATE_UNAVAILABLE",
        message: "IPC command failed.",
        details: null,
        name: "IpcCommandError",
        retryable: true,
      }),
    );
  });
});
