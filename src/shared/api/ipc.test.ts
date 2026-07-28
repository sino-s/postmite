import { beforeEach, describe, expect, it, vi } from "vitest";

const invokeMock = vi.hoisted(() => vi.fn());

vi.mock("@tauri-apps/api/core", () => ({
  invoke: invokeMock,
}));

import { invokeCommand, requestIpc, workspaceIpc } from "./ipc";

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

  it("invokes request commands with ordered duplicate fields intact", async () => {
    invokeMock.mockResolvedValue({
      workspaceId: "workspace-1",
      savedRequests: [],
      drafts: [],
      tabs: [],
    });

    await requestIpc.createSavedRequest({
      workspaceId: "workspace-1",
      content: {
        name: "Duplicate fields",
        method: "GET",
        url: "https://example.test",
        body: { type: "NONE" },
        query: [
          { enabled: true, order: 0, name: "tag", value: "first" },
          { enabled: false, order: 1, name: "tag", value: "" },
        ],
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
      },
    });

    expect(invokeMock).toHaveBeenCalledWith("create_saved_request", {
      input: {
        workspaceId: "workspace-1",
        content: {
          name: "Duplicate fields",
          method: "GET",
          url: "https://example.test",
          body: { type: "NONE" },
          query: [
            { enabled: true, order: 0, name: "tag", value: "first" },
            { enabled: false, order: 1, name: "tag", value: "" },
          ],
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
        },
      },
    });
  });

  it("invokes cookie jar commands through typed IPC", async () => {
    invokeMock.mockResolvedValue({
      workspaceId: "workspace-1",
      cookies: [],
    });

    await requestIpc.upsertCookie({
      workspaceId: "workspace-1",
      cookieId: null,
      name: "sid",
      value: "cookie-value",
      domain: "example.test",
      path: "/",
      secure: true,
      httpOnly: true,
      sameSite: "LAX",
      expiresAtEpochSeconds: null,
    });

    expect(invokeMock).toHaveBeenCalledWith("upsert_cookie", {
      input: {
        workspaceId: "workspace-1",
        cookieId: null,
        name: "sid",
        value: "cookie-value",
        domain: "example.test",
        path: "/",
        secure: true,
        httpOnly: true,
        sameSite: "LAX",
        expiresAtEpochSeconds: null,
      },
    });
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
