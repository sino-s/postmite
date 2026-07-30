import { useState, type KeyboardEvent, type ReactNode } from "react";

import {
  ResizableHandle,
  ResizablePanel,
  ResizablePanelGroup,
} from "../../../components/ui/resizable";
import type { RequestResponseSplit } from "../../../app/preferences";
import type { ResponseExecutionState } from "../../../shared/api/execution";
import type {
  ExecutionHistorySnapshotDto,
  ExecutionRecordDto,
  RequestContentDto,
  ResolvedRequestContentDto,
  WorkspaceCookieDto,
} from "../../../shared/api/generated/ipc";
import { FieldTable } from "../controls/FieldTable";
import { applyQueryToUrl } from "../models/ordered-fields";
import type { CookieFormValue } from "../models/request-editor-model";
import { BodyEditor } from "../panels/BodyEditor";
import { CookiePanel } from "../panels/CookiePanel";
import { HistoryPanel } from "../panels/HistoryPanel";
import { ResolutionPanel } from "../panels/ResolutionPanel";
import { ResponsePanel } from "../panels/ResponsePanel";
import { SecurityPanel } from "../panels/SecurityPanel";

type RequestEditorPanelsProps = {
  content: RequestContentDto;
  cookies: WorkspaceCookieDto[];
  cookiesLoading: boolean;
  execution: ResponseExecutionState | null;
  history: ExecutionHistorySnapshotDto | null;
  historyLoading: boolean;
  onChange: (updater: (content: RequestContentDto) => RequestContentDto) => void;
  onClearCookies: () => void;
  onDeleteCookie: (cookie: WorkspaceCookieDto) => void;
  onOpenHistoryRecord: (record: ExecutionRecordDto) => void;
  onRevealCookie: (cookie: WorkspaceCookieDto) => Promise<{ value: string }>;
  onSaveCookie: (input: CookieFormValue) => void;
  onToggleHistoryDisabled: (disabled: boolean) => void;
  onToggleHistoryPinned: (record: ExecutionRecordDto) => void;
  requestResponseSplit: RequestResponseSplit;
  resizable: boolean;
  resolution: ResolvedRequestContentDto | null;
  resolving: boolean;
  workspaceId: string;
};

export function RequestEditorPanels({
  content,
  cookies,
  cookiesLoading,
  execution,
  history,
  historyLoading,
  onChange,
  onClearCookies,
  onDeleteCookie,
  onOpenHistoryRecord,
  onRevealCookie,
  onSaveCookie,
  onToggleHistoryDisabled,
  onToggleHistoryPinned,
  requestResponseSplit,
  resizable,
  resolution,
  resolving,
  workspaceId,
}: RequestEditorPanelsProps) {
  const requestOptions = (
    <RequestOptionsTabs
      tabs={[
        {
          content: (
            <FieldTable
              fields={content.query}
              legend="Params"
              onChange={(fields) =>
                onChange((current) => ({
                  ...current,
                  query: fields,
                  url: applyQueryToUrl(current.url, fields),
                }))
              }
            />
          ),
          label: "Params",
          value: "params",
        },
        {
          content: (
            <SecurityPanel
              content={content}
              onChange={onChange}
              resolution={resolution}
            />
          ),
          label: "Authorization",
          value: "authorization",
        },
        {
          content: (
            <FieldTable
              fields={content.headers}
              legend="Headers"
              onChange={(fields) =>
                onChange((current) => ({
                  ...current,
                  headers: fields,
                }))
              }
            />
          ),
          label: "Headers",
          value: "headers",
        },
        {
          content: (
            <BodyEditor
              body={content.body}
              workspaceId={workspaceId}
              onChange={(body) =>
                onChange((current) => ({
                  ...current,
                  body,
                }))
              }
            />
          ),
          label: "Body",
          value: "body",
        },
        {
          content: <ResolutionPanel resolution={resolution} resolving={resolving} />,
          label: "Variables",
          value: "variables",
        },
        {
          content: (
            <HistoryPanel
              history={history}
              loading={historyLoading}
              onOpen={onOpenHistoryRecord}
              onToggleDisabled={onToggleHistoryDisabled}
              onTogglePinned={onToggleHistoryPinned}
            />
          ),
          label: "History",
          value: "history",
        },
        {
          content: (
            <CookiePanel
              cookies={cookies}
              loading={cookiesLoading}
              onClear={onClearCookies}
              onDelete={onDeleteCookie}
              onReveal={onRevealCookie}
              onSave={onSaveCookie}
            />
          ),
          label: "Cookies",
          value: "cookies",
        },
      ]}
    />
  );

  if (!resizable) {
    return (
      <div className="grid min-h-0 flex-1 grid-rows-[minmax(0,1fr)_minmax(0,1fr)] gap-4 overflow-hidden">
        {requestOptions}
        <ResponsePanel execution={execution} />
      </div>
    );
  }

  const panelOrientation = requestResponseSplit === "horizontal" ? "vertical" : "horizontal";
  const requestDefaultSize = requestResponseSplit === "horizontal" ? 52 : 56;
  const responseDefaultSize = 100 - requestDefaultSize;

  return (
    <ResizablePanelGroup
      className="min-h-0 flex-1 rounded-md border border-border bg-background"
      orientation={panelOrientation}
    >
      <ResizablePanel
        className="overflow-hidden"
        defaultSize={requestDefaultSize}
        minSize="260px"
      >
        <div className="h-full min-h-0 overflow-hidden p-4">
          {requestOptions}
        </div>
      </ResizablePanel>
      <ResizableHandle
        aria-label="Resize request and response panels"
        orientation={panelOrientation}
        withHandle
      />
      <ResizablePanel
        className="overflow-hidden"
        defaultSize={responseDefaultSize}
        minSize="220px"
      >
        <div className="h-full min-h-0 overflow-hidden p-4">
          <ResponsePanel execution={execution} />
        </div>
      </ResizablePanel>
    </ResizablePanelGroup>
  );
}

type RequestOptionsTabsProps = {
  tabs: Array<{
    content: ReactNode;
    label: string;
    value: string;
  }>;
};

function RequestOptionsTabs({ tabs }: RequestOptionsTabsProps) {
  const [activeTab, setActiveTab] = useState(tabs[0]?.value ?? "");
  const activePanel = tabs.find((tab) => tab.value === activeTab) ?? tabs[0];

  function activateTab(value: string) {
    setActiveTab(value);
    document.getElementById(`request-option-tab-${value}`)?.focus();
  }

  function handleTabKeyDown(event: KeyboardEvent<HTMLButtonElement>, index: number) {
    const lastIndex = tabs.length - 1;
    const nextIndexByKey: Record<string, number> = {
      ArrowLeft: index === 0 ? lastIndex : index - 1,
      ArrowRight: index === lastIndex ? 0 : index + 1,
      End: lastIndex,
      Home: 0,
    };
    const nextIndex = nextIndexByKey[event.key];
    if (nextIndex === undefined) {
      return;
    }
    event.preventDefault();
    activateTab(tabs[nextIndex].value);
  }

  return (
    <section aria-label="Request options" className="flex h-full min-h-0 min-w-0 flex-col overflow-hidden">
      <div className="flex h-full min-h-0 min-w-0 flex-col overflow-hidden">
        <div className="flex shrink-0 min-w-0 items-center">
          <div
            aria-label="Request option tabs"
            className="inline-flex h-auto min-w-0 max-w-full items-center justify-start overflow-x-auto rounded-md bg-muted p-1 text-muted-foreground"
            role="tablist"
          >
            {tabs.map((tab, index) => (
              <button
                aria-controls={`request-option-${tab.value}`}
                aria-selected={activeTab === tab.value}
                className="inline-flex min-h-7 items-center justify-center whitespace-nowrap rounded-sm px-3 py-1 text-sm font-medium transition-all focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-ring data-[state=active]:bg-background data-[state=active]:text-foreground data-[state=active]:shadow-sm"
                data-state={activeTab === tab.value ? "active" : "inactive"}
                id={`request-option-tab-${tab.value}`}
                key={tab.value}
                onClick={() => activateTab(tab.value)}
                onKeyDown={(event) => handleTabKeyDown(event, index)}
                role="tab"
                tabIndex={activeTab === tab.value ? 0 : -1}
                type="button"
              >
                {tab.label}
              </button>
            ))}
          </div>
        </div>
        <div
          aria-labelledby={`request-option-tab-${activePanel.value}`}
          className="mt-2 min-h-0 min-w-0 flex-1 overflow-auto focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-0 focus-visible:outline-ring [&>section]:min-h-0"
          id={`request-option-${activePanel.value}`}
          role="tabpanel"
          tabIndex={0}
        >
          {activePanel.content}
        </div>
      </div>
    </section>
  );
}
