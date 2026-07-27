import { beforeEach, describe, expect, it, vi } from "vitest";

import type { ExecutionEventDto } from "./generated/ipc";

const listenMock = vi.hoisted(() => vi.fn());
const requestIpcMock = vi.hoisted(() => ({
  cancelRequestExecution: vi.fn(),
  startRequestExecution: vi.fn(),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: listenMock,
}));

vi.mock("./ipc", () => ({
  requestIpc: requestIpcMock,
}));

import {
  REQUEST_EXECUTION_EVENT,
  cancelRequestExecution,
  listenToRequestExecutionEvents,
  reduceRequestExecutionEvent,
  startRequestExecution,
} from "./execution";

describe("request execution API", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("starts execution through typed Rust IPC", async () => {
    requestIpcMock.startRequestExecution.mockResolvedValue({
      executionId: "execution-1",
    });

    await expect(
      startRequestExecution({
        workspaceId: "workspace-1",
        draftId: "draft-1",
        content: {
          name: "GET users",
          method: "GET",
          url: "http://127.0.0.1/users",
          body: "",
          query: [],
          headers: [],
        },
      }),
    ).resolves.toEqual({
      status: "queued",
      executionId: "execution-1",
    });

    expect(requestIpcMock.startRequestExecution).toHaveBeenCalledWith({
      workspaceId: "workspace-1",
      draftId: "draft-1",
      content: {
        name: "GET users",
        method: "GET",
        url: "http://127.0.0.1/users",
        body: "",
        query: [],
        headers: [],
      },
    });
  });

  it("cancels execution through typed Rust IPC", async () => {
    requestIpcMock.cancelRequestExecution.mockResolvedValue({
      executionId: "execution-1",
      cancelled: true,
    });

    await expect(
      cancelRequestExecution({ executionId: "execution-1" }),
    ).resolves.toEqual({
      executionId: "execution-1",
      cancelled: true,
    });

    expect(requestIpcMock.cancelRequestExecution).toHaveBeenCalledWith({
      executionId: "execution-1",
    });
  });

  it("subscribes to request execution events from Tauri", async () => {
    const unlisten = vi.fn();
    listenMock.mockResolvedValue(unlisten);
    const onEvent = vi.fn();
    const payload = event("execution-1", 1n, "STARTED");

    const result = await listenToRequestExecutionEvents(onEvent);
    listenMock.mock.calls[0][1]({ payload });

    expect(result).toBe(unlisten);
    expect(listenMock).toHaveBeenCalledWith(
      REQUEST_EXECUTION_EVENT,
      expect.any(Function),
    );
    expect(onEvent).toHaveBeenCalledWith(payload);
  });

  it("ignores stale and out-of-order execution events", () => {
    const started = reduceRequestExecutionEvent(
      {
        currentExecutionId: "execution-1",
        lastSequence: 0n,
        latestEvent: null,
      },
      event("execution-1", 2n, "RESPONSE_HEADERS"),
    );

    const stale = reduceRequestExecutionEvent(
      started,
      event("execution-older", 3n, "COMPLETED"),
    );
    const outOfOrder = reduceRequestExecutionEvent(
      stale,
      event("execution-1", 1n, "STARTED"),
    );

    expect(stale).toBe(started);
    expect(outOfOrder).toBe(started);
    expect(started.latestEvent?.kind.type).toBe("RESPONSE_HEADERS");
  });
});

function event(
  executionId: string,
  sequence: bigint,
  type: ExecutionEventDto["kind"]["type"],
): ExecutionEventDto {
  if (type === "STARTED") {
    return {
      executionId,
      sequence,
      kind: { type, method: "GET", url: "http://127.0.0.1" },
    };
  }
  if (type === "RESPONSE_HEADERS") {
    return {
      executionId,
      sequence,
      kind: { type, status: 200, headers: [] },
    };
  }
  return {
    executionId,
    sequence,
    kind: { type },
  } as ExecutionEventDto;
}
