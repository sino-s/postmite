import {
  ResizableHandle,
  ResizablePanel,
  ResizablePanelGroup,
} from "../../../components/ui/resizable";
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
  resizable,
  resolution,
  resolving,
  workspaceId,
}: RequestEditorPanelsProps) {
  const requestConfiguration = (
    <section className="flex shrink-0 flex-col gap-4">
      <SecurityPanel
        content={content}
        onChange={onChange}
        resolution={resolution}
      />
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
    </section>
  );

  const bodyEditor = (
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
  );

  const resultPanels = (
    <>
      <ResolutionPanel resolution={resolution} resolving={resolving} />
      <ResponsePanel execution={execution} />
      <HistoryPanel
        history={history}
        loading={historyLoading}
        onOpen={onOpenHistoryRecord}
        onToggleDisabled={onToggleHistoryDisabled}
        onTogglePinned={onToggleHistoryPinned}
      />
      <CookiePanel
        cookies={cookies}
        loading={cookiesLoading}
        onClear={onClearCookies}
        onDelete={onDeleteCookie}
        onReveal={onRevealCookie}
        onSave={onSaveCookie}
      />
    </>
  );

  if (!resizable) {
    return (
      <div className="grid min-h-0 gap-4">
        {requestConfiguration}
        {bodyEditor}
        {resultPanels}
      </div>
    );
  }

  return (
    <ResizablePanelGroup
      className="min-h-0 flex-1 rounded-md border border-border bg-background"
      orientation="vertical"
    >
      <ResizablePanel className="overflow-hidden" defaultSize="58" minSize="280px">
        <div className="grid h-full min-h-0 gap-4 overflow-auto p-4 2xl:grid-cols-[minmax(0,1fr)_minmax(360px,0.9fr)]">
          {requestConfiguration}
          {bodyEditor}
        </div>
      </ResizablePanel>
      <ResizableHandle
        aria-label="Resize request and response panels"
        orientation="vertical"
        withHandle
      />
      <ResizablePanel className="overflow-hidden" defaultSize="42" minSize="220px">
        <div className="grid h-full min-h-0 gap-4 overflow-auto p-4 xl:grid-cols-[minmax(260px,0.6fr)_minmax(0,1fr)]">
          {resultPanels}
        </div>
      </ResizablePanel>
    </ResizablePanelGroup>
  );
}
