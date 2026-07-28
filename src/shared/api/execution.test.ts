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
  createQueuedResponseExecutionState,
  listenToRequestExecutionEvents,
  reduceRequestExecutionEvent,
  reduceResponseExecutionStates,
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
          body: { type: "NONE" },
          query: [],
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
        body: { type: "NONE" },
        query: [],
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

  it("reduces response events into bounded per-draft view state", () => {
    const queued = createQueuedResponseExecutionState({
      draftId: "draft-1",
      executionId: "execution-1",
      nowMs: 100,
    });
    const states = reduceResponseExecutionStates(
      { "draft-1": queued },
      {
        executionId: "execution-1",
        sequence: 1n,
        kind: {
          type: "COMPLETED",
          status: 200,
          bodyPreview: "{\"ok\":true}",
          bodyTruncated: false,
          decodedBytes: 11n,
          wireBytes: 31n,
        },
      },
      145,
    );

    expect(states["draft-1"]).toMatchObject({
      phase: "completed",
      status: 200,
      bodyPreview: "{\"ok\":true}",
      bodyTruncated: false,
      completedAtMs: 145,
    });
  });

  it("keeps TLS visibility and redirect chain in response state", () => {
    const queued = createQueuedResponseExecutionState({
      draftId: "draft-1",
      executionId: "execution-1",
      nowMs: 100,
    });
    const started = reduceResponseExecutionStates(
      { "draft-1": queued },
      {
        executionId: "execution-1",
        sequence: 1n,
        kind: {
          type: "STARTED",
          method: "POST",
          url: "https://example.test/login",
          tlsVerification: false,
          proxy: defaultProxyMetadata(),
          timeouts: defaultTimeoutMetadata(),
        },
      },
      110,
    );
    const redirected = reduceResponseExecutionStates(
      started,
      {
        executionId: "execution-1",
        sequence: 2n,
        kind: {
          type: "REDIRECTED",
          from: "https://example.test/login",
          to: "https://example.test/session",
          status: 303,
        },
      },
      120,
    );

    expect(redirected["draft-1"]).toMatchObject({
      method: "POST",
      url: "https://example.test/login",
      tlsVerification: false,
      redirects: [
        {
          from: "https://example.test/login",
          to: "https://example.test/session",
          status: 303,
        },
      ],
    });
  });

  it("ignores response events for another execution or stale sequence", () => {
    const queued = createQueuedResponseExecutionState({
      draftId: "draft-1",
      executionId: "execution-1",
      nowMs: 100,
    });
    const states = { "draft-1": queued };

    expect(
      reduceResponseExecutionStates(
        states,
        event("execution-older", 1n, "COMPLETED"),
        150,
      ),
    ).toBe(states);

    const completed = reduceResponseExecutionStates(
      states,
      event("execution-1", 2n, "COMPLETED"),
      150,
    );
    expect(
      reduceResponseExecutionStates(
        completed,
        event("execution-1", 1n, "FAILED"),
        160,
      ),
    ).toBe(completed);
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
      kind: {
        type,
        method: "GET",
        url: "http://127.0.0.1",
        tlsVerification: true,
        proxy: defaultProxyMetadata(),
        timeouts: defaultTimeoutMetadata(),
      },
    };
  }
  if (type === "REDIRECTED") {
    return {
      executionId,
      sequence,
      kind: {
        type,
        from: "http://127.0.0.1",
        to: "http://127.0.0.1/next",
        status: 302,
      },
    };
  }
  if (type === "RESPONSE_HEADERS") {
    return {
      executionId,
      sequence,
      kind: {
        type,
        status: 200,
        headers: [],
        protocol: "HTTP/1.1",
        remoteAddr: "127.0.0.1:8080",
      },
    };
  }
  if (type === "COMPLETED") {
    return {
      executionId,
      sequence,
      kind: {
        type,
        status: 200,
        bodyPreview: "",
        bodyTruncated: false,
        decodedBytes: 0n,
        wireBytes: null,
      },
    };
  }
  return {
    executionId,
    sequence,
    kind: { type },
  } as ExecutionEventDto;
}

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

function defaultProxyMetadata() {
  return {
    source: "processEnvironment",
    selectedProxy: null,
    bypassReason: null,
  };
}

function defaultTimeoutMetadata() {
  return {
    connectMs: 10_000n,
    overallMs: 300_000n,
    idleMs: 60_000n,
  };
}
