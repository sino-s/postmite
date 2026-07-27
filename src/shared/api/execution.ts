import type { RequestContentDto } from "./generated/ipc";

export type PlaceholderExecutionRequest = {
  workspaceId: string;
  draftId: string;
  content: RequestContentDto;
};

export type PlaceholderExecutionResult = {
  status: "queued";
  executionId: string;
};

export async function requestPlaceholderExecution(
  input: PlaceholderExecutionRequest,
): Promise<PlaceholderExecutionResult> {
  return {
    status: "queued",
    executionId: `placeholder:${input.workspaceId}:${input.draftId}`,
  };
}
