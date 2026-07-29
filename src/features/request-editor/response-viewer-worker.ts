import {
  prepareStructuredViewer,
  type StructuredViewerInput,
} from "./response-viewer-worker-core";

self.addEventListener("message", (event: MessageEvent<StructuredViewerInput>) => {
  self.postMessage(prepareStructuredViewer(event.data));
});
