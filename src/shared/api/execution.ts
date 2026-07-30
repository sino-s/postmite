import { listen } from "@tauri-apps/api/event";

import type {
  CancelRequestExecutionInput,
  ExecutionEventDto,
  ExecutionProxyMetadataDto,
  FrontendExecutionTraceStageDto,
  ResponseFileMetadataDto,
  SaveResponseFileInput,
  ExecutionTimingMetadataDto,
  ExecutionTimeoutMetadataDto,
  StartRequestExecutionInput,
} from "./generated/ipc";
import { requestIpc } from "./ipc";

export const REQUEST_EXECUTION_EVENT = "request_execution_event";
export type { ExecutionEventDto };

export type RequestExecutionResult = {
  status: "queued";
  executionId: string;
  initialEvents: ExecutionEventDto[];
};

export type RequestExecutionState = {
  currentExecutionId: string | null;
  lastSequence: bigint;
  latestEvent: ExecutionEventDto | null;
};

export type ResponseExecutionPhase =
  | "idle"
  | "running"
  | "completed"
  | "failed"
  | "cancelled";

export type ResponseExecutionState = {
  draftId: string;
  executionId: string;
  phase: ResponseExecutionPhase;
  startedAtMs: number;
  completedAtMs: number | null;
  lastSequence: bigint;
  method: string | null;
  url: string | null;
  tlsVerification: boolean | null;
  proxy: ExecutionProxyMetadataDto | null;
  timeouts: ExecutionTimeoutMetadataDto | null;
  timing: ExecutionTimingMetadataDto;
  redirects: Array<{ from: string; to: string; status: number }>;
  status: number | null;
  protocol: string | null;
  remoteAddr: string | null;
  headers: Array<{ name: string; value: string }>;
  bodyPreview: string;
  bodyTruncated: boolean;
  decodedBytes: bigint | null;
  wireBytes: bigint | null;
  responseFile: ResponseFileMetadataDto | null;
  error: string | null;
  uploadProgress: { sentBytes: bigint; totalBytes: bigint } | null;
  downloadProgress: { receivedBytes: bigint; totalBytes: bigint | null } | null;
};

export type ResponseExecutionStateByDraft = Record<
  string,
  ResponseExecutionState
>;

export async function startRequestExecution(
  input: StartRequestExecutionInput,
): Promise<RequestExecutionResult> {
  void recordFrontendExecutionTrace(input.executionId, "START_REQUESTED");
  try {
    const result = await requestIpc.startRequestExecution(input);
    void recordFrontendExecutionTrace(input.executionId, "START_RESOLVED");
    return {
      status: "queued",
      executionId: result.executionId,
      initialEvents: result.initialEvents ?? [],
    };
  } catch (error) {
    void recordFrontendExecutionTrace(input.executionId, "START_REJECTED");
    throw error;
  }
}

export async function cancelRequestExecution(
  input: CancelRequestExecutionInput,
) {
  return requestIpc.cancelRequestExecution(input);
}

export async function saveResponseFile(input: SaveResponseFileInput) {
  return requestIpc.saveResponseFile(input);
}

export async function listenToRequestExecutionEvents(
  onEvent: (event: ExecutionEventDto) => void,
) {
  return listen<ExecutionEventDto>(REQUEST_EXECUTION_EVENT, (event) => {
    if (
      event.payload.kind.type === "STARTED" ||
      event.payload.kind.type === "RESPONSE_HEADERS" ||
      event.payload.kind.type === "COMPLETED" ||
      event.payload.kind.type === "FAILED" ||
      event.payload.kind.type === "CANCELLED"
    ) {
      void recordFrontendExecutionTrace(
        event.payload.executionId,
        "EVENT_RECEIVED",
        event.payload.sequence,
      );
    }
    onEvent(event.payload);
  });
}

export async function recordFrontendExecutionTrace(
  executionId: string,
  stage: FrontendExecutionTraceStageDto,
  sequence: bigint | null = null,
) {
  try {
    await requestIpc.recordFrontendExecutionTrace({
      executionId,
      stage,
      sequence,
    });
  } catch {
    // Diagnostics must never interrupt request execution.
  }
}

type ExecutionCrypto = Pick<Crypto, "getRandomValues"> &
  Partial<Pick<Crypto, "randomUUID">>;

export function createExecutionId(cryptoApi: ExecutionCrypto = globalThis.crypto): string {
  if (typeof cryptoApi.randomUUID === "function") {
    return cryptoApi.randomUUID();
  }

  const bytes = cryptoApi.getRandomValues(new Uint8Array(16));
  bytes[6] = (bytes[6] & 0x0f) | 0x40;
  bytes[8] = (bytes[8] & 0x3f) | 0x80;
  const hex = Array.from(bytes, (byte) => byte.toString(16).padStart(2, "0"));
  return [
    hex.slice(0, 4).join(""),
    hex.slice(4, 6).join(""),
    hex.slice(6, 8).join(""),
    hex.slice(8, 10).join(""),
    hex.slice(10, 16).join(""),
  ].join("-");
}

export function reduceRequestExecutionEvent(
  state: RequestExecutionState,
  event: ExecutionEventDto,
): RequestExecutionState {
  if (state.currentExecutionId !== event.executionId) {
    return state;
  }

  if (event.sequence <= state.lastSequence) {
    return state;
  }

  return {
    ...state,
    lastSequence: event.sequence,
    latestEvent: event,
  };
}

export function createQueuedResponseExecutionState({
  draftId,
  executionId,
  nowMs,
}: {
  draftId: string;
  executionId: string;
  nowMs: number;
}): ResponseExecutionState {
  return {
    draftId,
    executionId,
    phase: "running",
    startedAtMs: nowMs,
    completedAtMs: null,
    lastSequence: 0n,
    method: null,
    url: null,
    tlsVerification: null,
    proxy: null,
    timeouts: null,
    timing: emptyTiming(),
    redirects: [],
    status: null,
    protocol: null,
    remoteAddr: null,
    headers: [],
    bodyPreview: "",
    bodyTruncated: false,
    decodedBytes: null,
    wireBytes: null,
    responseFile: null,
    error: null,
    uploadProgress: null,
    downloadProgress: null,
  };
}

export function reduceResponseExecutionStates(
  states: ResponseExecutionStateByDraft,
  event: ExecutionEventDto,
  nowMs: number,
): ResponseExecutionStateByDraft {
  const draftId = Object.keys(states).find(
    (key) => states[key]?.executionId === event.executionId,
  );
  if (!draftId) {
    return states;
  }

  const current = states[draftId];
  if (event.sequence <= current.lastSequence) {
    return states;
  }

  const next = reduceResponseExecutionState(current, event, nowMs);
  if (next === current) {
    return states;
  }

  return {
    ...states,
    [draftId]: next,
  };
}

export function applyResponseExecutionEvents(
  state: ResponseExecutionState,
  events: ExecutionEventDto[],
  nowMs: number,
): ResponseExecutionState {
  return events.reduce(
    (current, event) => reduceResponseExecutionState(current, event, nowMs),
    state,
  );
}

export function reduceResponseExecutionState(
  state: ResponseExecutionState,
  event: ExecutionEventDto,
  nowMs: number,
): ResponseExecutionState {
  if (state.executionId !== event.executionId || event.sequence <= state.lastSequence) {
    return state;
  }

  const base = { ...state, lastSequence: event.sequence };
  switch (event.kind.type) {
    case "STARTED":
      return {
        ...base,
        phase: "running",
        method: event.kind.method,
        url: event.kind.url,
        tlsVerification: event.kind.tlsVerification,
        proxy: event.kind.proxy,
        timeouts: event.kind.timeouts,
        timing: {
          ...base.timing,
          queuedMs: event.kind.queuedMs,
        },
      };
    case "REDIRECTED":
      return {
        ...base,
        redirects: [
          ...base.redirects,
          {
            from: event.kind.from,
            to: event.kind.to,
            status: event.kind.status,
          },
        ],
      };
    case "UPLOAD_PROGRESS":
      return {
        ...base,
        uploadProgress: {
          sentBytes: event.kind.sentBytes,
          totalBytes: event.kind.totalBytes,
        },
      };
    case "RESPONSE_HEADERS":
      return {
        ...base,
        status: event.kind.status,
        protocol: event.kind.protocol,
        remoteAddr: event.kind.remoteAddr,
        headers: event.kind.headers,
      };
    case "DOWNLOAD_PROGRESS":
      return {
        ...base,
        downloadProgress: {
          receivedBytes: event.kind.receivedBytes,
          totalBytes: event.kind.totalBytes,
        },
      };
    case "COMPLETED":
      return {
        ...base,
        phase: "completed",
        completedAtMs: nowMs,
        status: event.kind.status,
        bodyPreview: event.kind.bodyPreview,
        bodyTruncated: event.kind.bodyTruncated,
        decodedBytes: event.kind.decodedBytes,
        wireBytes: event.kind.wireBytes,
        responseFile: event.kind.responseFile,
        timing: event.kind.timing,
        error: null,
      };
    case "FAILED":
      return {
        ...base,
        phase: "failed",
        completedAtMs: nowMs,
        error: event.kind.message,
      };
    case "CANCELLED":
      return {
        ...base,
        phase: "cancelled",
        completedAtMs: nowMs,
      };
  }
}

function emptyTiming(): ExecutionTimingMetadataDto {
  return {
    queuedMs: 0n,
    dnsMs: null,
    connectMs: null,
    tlsMs: null,
    firstByteMs: null,
    downloadMs: null,
    totalMs: 0n,
  };
}

export function isTerminalResponseExecution(state: ResponseExecutionState) {
  return (
    state.phase === "completed" ||
    state.phase === "failed" ||
    state.phase === "cancelled"
  );
}
