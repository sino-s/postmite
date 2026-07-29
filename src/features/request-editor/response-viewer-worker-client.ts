import {
  prepareStructuredViewer,
  type StructuredViewerInput,
  type StructuredViewerResult,
} from "./response-viewer-worker-core";

export function prepareStructuredViewerAsync(
  input: StructuredViewerInput,
): Promise<StructuredViewerResult> {
  if (typeof Worker === "undefined") {
    return Promise.resolve(prepareStructuredViewer(input));
  }

  return new Promise((resolve) => {
    const worker = new Worker(new URL("./response-viewer-worker.ts", import.meta.url), {
      type: "module",
    });
    worker.addEventListener(
      "message",
      (event: MessageEvent<StructuredViewerResult>) => {
        worker.terminate();
        resolve(event.data);
      },
      { once: true },
    );
    worker.addEventListener(
      "error",
      () => {
        worker.terminate();
        resolve(prepareStructuredViewer(input));
      },
      { once: true },
    );
    worker.postMessage(input);
  });
}
