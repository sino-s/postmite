import { listen } from "@tauri-apps/api/event";

import type {
  CancelRequestExecutionInput,
  ExecutionEventDto,
  StartRequestExecutionInput,
} from "./generated/ipc";
import { requestIpc } from "./ipc";

export const REQUEST_EXECUTION_EVENT = "request_execution_event";

export type RequestExecutionResult = {
  status: "queued";
  executionId: string;
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
  redirects: Array<{ from: string; to: string; status: number }>;
  status: number | null;
  headers: Array<{ name: string; value: string }>;
  bodyPreview: string;
  bodyTruncated: boolean;
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
  const result = await requestIpc.startRequestExecution(input);
  return {
    status: "queued",
    executionId: result.executionId,
  };
}

export async function cancelRequestExecution(
  input: CancelRequestExecutionInput,
) {
  return requestIpc.cancelRequestExecution(input);
}

export async function listenToRequestExecutionEvents(
  onEvent: (event: ExecutionEventDto) => void,
) {
  return listen<ExecutionEventDto>(REQUEST_EXECUTION_EVENT, (event) => {
    onEvent(event.payload);
  });
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
    redirects: [],
    status: null,
    headers: [],
    bodyPreview: "",
    bodyTruncated: false,
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

export function isTerminalResponseExecution(state: ResponseExecutionState) {
  return (
    state.phase === "completed" ||
    state.phase === "failed" ||
    state.phase === "cancelled"
  );
}
