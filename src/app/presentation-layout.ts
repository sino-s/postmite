import type {
  ResizableLayout,
  ResizableLayoutChangedMeta,
} from "../components/ui/resizable";

import type { RequestResponseSplit } from "./preferences";

type LayoutStorage = Pick<Storage, "getItem" | "setItem">;

const layoutStorageKeys: Record<RequestResponseSplit, string> = {
  horizontal: "postmite.requestResponseLayout.horizontal",
  vertical: "postmite.requestResponseLayout.vertical",
};

const defaultLayouts: Record<RequestResponseSplit, ResizableLayout> = {
  horizontal: { request: 52, response: 48 },
  vertical: { request: 56, response: 44 },
};

function isBoundedLayout(value: unknown): value is ResizableLayout {
  if (!value || typeof value !== "object" || Array.isArray(value)) return false;

  const entries = Object.entries(value);
  if (
    entries.length !== 2
    || !Object.hasOwn(value, "request")
    || !Object.hasOwn(value, "response")
  ) {
    return false;
  }

  const { request, response } = value as Record<string, unknown>;
  return typeof request === "number"
    && Number.isFinite(request)
    && request >= 10
    && request <= 90
    && typeof response === "number"
    && Number.isFinite(response)
    && response >= 10
    && response <= 90
    && Math.abs(request + response - 100) < 0.01;
}

export function defaultRequestResponseLayout(split: RequestResponseSplit): ResizableLayout {
  return { ...defaultLayouts[split] };
}

export function loadRequestResponseLayout(
  storage: LayoutStorage,
  split: RequestResponseSplit,
): ResizableLayout {
  try {
    const stored = storage.getItem(layoutStorageKeys[split]);
    if (!stored) return defaultRequestResponseLayout(split);
    const parsed: unknown = JSON.parse(stored);
    return isBoundedLayout(parsed) ? parsed : defaultRequestResponseLayout(split);
  } catch {
    return defaultRequestResponseLayout(split);
  }
}

export function saveRequestResponseLayout(
  storage: LayoutStorage,
  split: RequestResponseSplit,
  layout: ResizableLayout,
  meta: ResizableLayoutChangedMeta,
) {
  if (!meta.isUserInteraction || !isBoundedLayout(layout)) return;

  try {
    storage.setItem(layoutStorageKeys[split], JSON.stringify(layout));
  } catch {
    // Presentation persistence must never make the editor unusable.
  }
}
