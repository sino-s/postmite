import { QueryClient } from "@tanstack/react-query";

export function createAppQueryClient() {
  return new QueryClient({
    defaultOptions: {
      queries: {
        retry: (failureCount, error) =>
          isRetryableError(error) && failureCount < 2,
        staleTime: 30_000,
      },
      mutations: {
        retry: (failureCount, error) =>
          isRetryableError(error) && failureCount < 1,
      },
    },
  });
}

export const queryClient = createAppQueryClient();

function isRetryableError(error: unknown) {
  return (
    typeof error === "object" &&
    error !== null &&
    "retryable" in error &&
    error.retryable === true
  );
}
