import { X } from "lucide-react";

import { Button } from "../../../components/ui/button";
import type { RequestDraftDto, RequestTabDto } from "../../../shared/api/generated/ipc";
import { isDraftDirty, type OverrideMap } from "../models/request-editor-model";

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
      className="flex min-h-11 items-stretch overflow-x-auto border-b border-border bg-background"
    >
      {tabs.map((tab) => {
        const dirty = isDraftDirty(tab.draftId, drafts, overrides);
        return (
          <div
            className="flex min-w-44 items-center border-r border-border"
            key={tab.id}
          >
            <Button
              aria-current={activeTabId === tab.id ? "page" : undefined}
              className="h-auto min-w-0 flex-1 justify-start rounded-none px-3 py-2 text-left text-foreground hover:bg-accent aria-[current=page]:bg-muted"
              onClick={() => onActivate(tab.id)}
              type="button"
              variant="ghost"
            >
              <span className="block truncate">
                {tab.title}
                {dirty ? " *" : ""}
              </span>
            </Button>
            <Button
              aria-label={`Close ${tab.title}`}
              className="mr-1 text-muted-foreground hover:text-foreground"
              onClick={() => onClose(tab)}
              size="icon"
              type="button"
              variant="ghost"
            >
              <X aria-hidden="true" size={16} />
            </Button>
          </div>
        );
      })}
    </nav>
  );
}
