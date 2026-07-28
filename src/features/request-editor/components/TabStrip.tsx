import { X } from "lucide-react";

import type { RequestDraftDto, RequestTabDto } from "../../../shared/api/generated/ipc";
import { isDraftDirty, type OverrideMap } from "../request-editor-model";

type TabStripProps = {
  activeTabId: string | null;
  drafts: RequestDraftDto[];
  onActivate: (tabId: string) => void;
  onClose: (tab: RequestTabDto) => void;
  tabs: RequestTabDto[];
  overrides: OverrideMap;
};

export function TabStrip({
  activeTabId,
  drafts,
  onActivate,
  onClose,
  overrides,
  tabs,
}: TabStripProps) {
  return (
    <nav
      aria-label="Request tabs"
      className="flex min-h-11 items-stretch overflow-x-auto border-b border-slate-300 bg-white"
    >
      {tabs.map((tab) => {
        const dirty = isDraftDirty(tab.draftId, drafts, overrides);
        return (
          <div
            className="flex min-w-44 items-center border-r border-slate-300"
            key={tab.id}
          >
            <button
              aria-current={activeTabId === tab.id ? "page" : undefined}
              className="min-w-0 flex-1 px-3 py-2 text-left text-sm hover:bg-slate-100 focus-visible:outline focus-visible:outline-2 focus-visible:outline-sky-500 aria-[current=page]:bg-sky-50"
              onClick={() => onActivate(tab.id)}
              type="button"
            >
              <span className="block truncate">
                {tab.title}
                {dirty ? " *" : ""}
              </span>
            </button>
            <button
              aria-label={`Close ${tab.title}`}
              className="mr-1 inline-flex h-8 w-8 items-center justify-center rounded-md text-slate-600 hover:bg-slate-100 focus-visible:outline focus-visible:outline-2 focus-visible:outline-sky-500"
              onClick={() => onClose(tab)}
              type="button"
            >
              <X aria-hidden="true" size={16} />
            </button>
          </div>
        );
      })}
    </nav>
  );
}
