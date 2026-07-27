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
