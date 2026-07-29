import { useEffect, useMemo, useState } from "react";

import { I18nProvider } from "./i18n";
import { PreferencesProvider } from "./preferences";
import { BodyEditor } from "../features/request-editor/components/BodyEditor";
import { CollectionsSidebar } from "../features/request-editor/components/CollectionsSidebar";
import { CookiePanel } from "../features/request-editor/components/CookiePanel";
import { FieldTable } from "../features/request-editor/components/FieldTable";
import { HistoryPanel } from "../features/request-editor/components/HistoryPanel";
import { RequestLine } from "../features/request-editor/components/RequestLine";
import { ResolutionPanel } from "../features/request-editor/components/ResolutionPanel";
import { ResponsePanel } from "../features/request-editor/components/ResponsePanel";
import { SecurityPanel } from "../features/request-editor/components/SecurityPanel";
import { TabStrip } from "../features/request-editor/components/TabStrip";
import { applyQueryToUrl } from "../features/request-editor/ordered-fields";
import { screenshotCookies, screenshotExecution, screenshotHistory, screenshotResolution, screenshotSnapshot } from "../features/request-editor/screenshot-fixtures";
import type { RequestContentDto } from "../shared/api/generated/ipc";

type ScreenshotVariant = {
  density: "comfortable" | "compact";
  state: "workspace" | "empty";
  theme: "light" | "dark";
};

export function ScreenshotApp() {
  return (
    <I18nProvider>
      <PreferencesProvider>
        <ScreenshotWorkspace />
      </PreferencesProvider>
    </I18nProvider>
  );
}

function ScreenshotWorkspace() {
  const variant = useMemo(readVariant, []);
  const [content, setContent] = useState<RequestContentDto>(
    screenshotSnapshot.drafts[0].content,
  );

  useEffect(() => {
    document.documentElement.dataset.theme = variant.theme;
    document.documentElement.dataset.resolvedTheme = variant.theme;
    document.documentElement.dataset.density = variant.density;
  }, [variant]);

  if (variant.state === "empty") {
    return <EmptyWorkspace variant={variant} />;
  }

  const activeTab = screenshotSnapshot.tabs[0];
  const activeDraft = screenshotSnapshot.drafts[0];

  return (
    <main className="flex min-h-screen flex-col bg-slate-100 text-slate-950">
      <header className="flex min-h-12 items-center justify-between border-b border-slate-300 bg-white px-4">
        <h1 className="text-sm font-semibold">Postmite</h1>
        <div className="flex items-center gap-2 text-xs text-slate-600">
          <span>Screenshot fixture</span>
          <span>{variant.theme}</span>
          <span>{variant.density}</span>
        </div>
      </header>
      <div className="grid min-h-0 flex-1 grid-cols-1 md:grid-cols-[280px_minmax(0,1fr)]">
        <CollectionsSidebar
          environments={screenshotSnapshot.environments}
          folders={screenshotSnapshot.collectionFolders}
          onCreateFolder={() => undefined}
          onDeleteFolder={() => undefined}
          onDeleteRequest={() => undefined}
          onDuplicateFolder={() => undefined}
          onDuplicateRequest={() => undefined}
          onMoveFolder={() => undefined}
          onMoveRequest={() => undefined}
          onOpenRequest={() => undefined}
          onRenameFolder={() => undefined}
          onSelectEnvironment={() => undefined}
          requests={screenshotSnapshot.savedRequests}
        />
        <div className="flex min-h-0 flex-col">
          <TabStrip
            activeTabId={activeTab.id}
            drafts={[{ ...activeDraft, content }]}
            onActivate={() => undefined}
            onClose={() => undefined}
            overrides={{}}
            tabs={screenshotSnapshot.tabs}
          />
          <section
            aria-label="Request editor screenshot fixture"
            className="grid min-h-0 flex-1 grid-rows-[auto_minmax(0,1fr)_auto] gap-4 p-4"
          >
            <RequestLine
              content={content}
              executionPhase="completed"
              executionRunning={false}
              onCancel={() => undefined}
              onChange={(updater) => setContent((current) => updater(current))}
              onExecute={() => undefined}
              onSave={() => undefined}
              saving={false}
            />
            <div className="grid min-h-0 gap-4 lg:grid-cols-[minmax(0,1fr)_minmax(360px,0.9fr)]">
              <section className="flex min-h-0 flex-col gap-4">
                <SecurityPanel
                  content={content}
                  onChange={(updater) => setContent((current) => updater(current))}
                  resolution={screenshotResolution}
                />
                <FieldTable
                  fields={content.query}
                  legend="Params"
                  onChange={(fields) =>
                    setContent((current) => ({
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
                    setContent((current) => ({ ...current, headers: fields }))
                  }
                />
              </section>
              <BodyEditor
                body={content.body}
                onChange={(body) => setContent((current) => ({ ...current, body }))}
                workspaceId={screenshotSnapshot.workspaceId}
              />
            </div>
            <div className="grid gap-4 xl:grid-cols-[minmax(260px,0.4fr)_minmax(0,1fr)_minmax(300px,0.5fr)_minmax(300px,0.5fr)]">
              <ResolutionPanel resolution={screenshotResolution} resolving={false} />
              <ResponsePanel execution={screenshotExecution} />
              <HistoryPanel
                history={screenshotHistory}
                loading={false}
                onOpen={() => undefined}
                onToggleDisabled={() => undefined}
                onTogglePinned={() => undefined}
              />
              <CookiePanel
                cookies={screenshotCookies}
                loading={false}
                onClear={() => undefined}
                onDelete={() => undefined}
                onReveal={async () => ({ value: "" })}
                onSave={() => undefined}
              />
            </div>
          </section>
        </div>
      </div>
    </main>
  );
}

function readVariant(): ScreenshotVariant {
  const params = new URLSearchParams(window.location.search);
  return {
    density: params.get("density") === "compact" ? "compact" : "comfortable",
    state: params.get("state") === "empty" ? "empty" : "workspace",
    theme: params.get("theme") === "dark" ? "dark" : "light",
  };
}

function EmptyWorkspace({ variant }: { variant: ScreenshotVariant }) {
  return (
    <main className="flex min-h-screen flex-col bg-slate-100 text-slate-950">
      <header className="flex min-h-12 items-center justify-between border-b border-slate-300 bg-white px-4">
        <h1 className="text-sm font-semibold">Postmite</h1>
        <div className="flex items-center gap-2 text-xs text-slate-600">
          <span>Screenshot fixture</span>
          <span>{variant.theme}</span>
          <span>{variant.density}</span>
          <span>empty</span>
        </div>
      </header>
      <div className="grid min-h-0 flex-1 grid-cols-1 md:grid-cols-[280px_minmax(0,1fr)]">
        <CollectionsSidebar
          environments={[]}
          folders={[]}
          onCreateFolder={() => undefined}
          onDeleteFolder={() => undefined}
          onDeleteRequest={() => undefined}
          onDuplicateFolder={() => undefined}
          onDuplicateRequest={() => undefined}
          onMoveFolder={() => undefined}
          onMoveRequest={() => undefined}
          onOpenRequest={() => undefined}
          onRenameFolder={() => undefined}
          onSelectEnvironment={() => undefined}
          requests={[]}
        />
        <div className="flex min-h-0 flex-col">
          <TabStrip
            activeTabId={null}
            drafts={[]}
            onActivate={() => undefined}
            onClose={() => undefined}
            overrides={{}}
            tabs={[]}
          />
          <section
            aria-label="Empty request workspace screenshot fixture"
            className="flex flex-1 items-center justify-center p-6"
          >
            <button
              className="inline-flex h-10 items-center gap-2 rounded-md bg-slate-900 px-4 text-sm font-medium text-white hover:bg-slate-700 focus-visible:outline focus-visible:outline-2 focus-visible:outline-sky-500"
              type="button"
            >
              New Request
            </button>
          </section>
        </div>
      </div>
    </main>
  );
}
