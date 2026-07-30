import { beforeEach, describe, expect, it, vi } from "vitest";

import type { ExecutionEventDto } from "./generated/ipc";

const listenMock = vi.hoisted(() => vi.fn());
const requestIpcMock = vi.hoisted(() => ({
  cancelRequestExecution: vi.fn(),
  saveResponseFile: vi.fn(),
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
  saveResponseFile,
  startRequestExecution,
} from "./execution";

describe("request execution API", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("starts execution through typed Rust IPC", async () => {
    requestIpcMock.startRequestExecution.mockResolvedValue({
      executionId: "execution-1",
      initialEvents: [event("execution-1", 1n, "STARTED")],
    });

    await expect(
      startRequestExecution({
        workspaceId: "workspace-1",
        draftId: "draft-1",
        executionId: "execution-1",
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
      initialEvents: [event("execution-1", 1n, "STARTED")],
    });

    expect(requestIpcMock.startRequestExecution).toHaveBeenCalledWith({
      workspaceId: "workspace-1",
      draftId: "draft-1",
      executionId: "execution-1",
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

  it("saves a spooled response file through typed Rust IPC", async () => {
    requestIpcMock.saveResponseFile.mockResolvedValue({
      destinationPath: "/home/sino/downloads/response.bin",
      byteCount: 4096n,
    });

    await expect(
      saveResponseFile({
        sourcePath: "/tmp/postmite-response-files/response.fixture.tmp",
        destinationPath: "/home/sino/downloads/response.bin",
      }),
    ).resolves.toEqual({
      destinationPath: "/home/sino/downloads/response.bin",
      byteCount: 4096n,
    });

    expect(requestIpcMock.saveResponseFile).toHaveBeenCalledWith({
      sourcePath: "/tmp/postmite-response-files/response.fixture.tmp",
      destinationPath: "/home/sino/downloads/response.bin",
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
          responseFile: null,
          timing: timingMetadata(),
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

  it("keeps spooled response file metadata separate from the preview", () => {
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
          bodyPreview: "leading preview",
          bodyTruncated: true,
          decodedBytes: 12_582_912n,
          wireBytes: 12_582_912n,
          responseFile: {
            path: "/tmp/postmite-response-files/response.fixture.tmp",
            byteCount: 12_582_912n,
            expiresAtEpochSeconds: 1_800_086_400n,
          },
          timing: timingMetadata(),
        },
      },
      145,
    );

    expect(states["draft-1"]).toMatchObject({
      bodyPreview: "leading preview",
      bodyTruncated: true,
      responseFile: {
        path: "/tmp/postmite-response-files/response.fixture.tmp",
        byteCount: 12_582_912n,
      },
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
          queuedMs: 7n,
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
      timing: expect.objectContaining({ queuedMs: 7n }),
    });
  });

  it("stores completed timing metadata in response state", () => {
    const queued = createQueuedResponseExecutionState({
      draftId: "draft-1",
      executionId: "execution-1",
      nowMs: 100,
    });
    const completed = reduceResponseExecutionStates(
      { "draft-1": queued },
      event("execution-1", 1n, "COMPLETED"),
      200,
    );

    expect(completed["draft-1"].timing).toEqual(timingMetadata());
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
        queuedMs: 0n,
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
        responseFile: null,
        timing: timingMetadata(),
      },
    };
  }
  return {
    executionId,
    sequence,
    kind: { type },
  } as ExecutionEventDto;
}

function timingMetadata() {
  return {
    queuedMs: 0n,
    dnsMs: null,
    connectMs: null,
    tlsMs: null,
    firstByteMs: 12n,
    downloadMs: 3n,
    totalMs: 20n,
  };
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
