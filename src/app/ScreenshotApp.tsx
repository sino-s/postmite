import { useEffect, useMemo, useState } from "react";

import { AppHeader } from "./AppHeader";
import { Button } from "../components/ui/button";
import {
  ResizableHandle,
  ResizablePanel,
  ResizablePanelGroup,
} from "../components/ui/resizable";
import { I18nProvider, useI18n } from "./i18n";
import { PreferencesProvider, usePreferences } from "./preferences";
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
  const isDesktopLayout = useDesktopLayout();
  const isEditorResizableLayout = useMediaQuery("(min-width: 1024px)", true);
  const variant = useMemo(readVariant, []);
  const { locale, setLocale } = useI18n();
  const { density, setDensity, setTheme, theme } = usePreferences();
  const [content, setContent] = useState<RequestContentDto>(
    screenshotSnapshot.drafts[0].content,
  );

  useEffect(() => {
    setTheme(variant.theme);
    setDensity(variant.density);
  }, [setDensity, setTheme, variant]);

  if (variant.state === "empty") {
    return <EmptyWorkspace variant={variant} />;
  }

  const activeTab = screenshotSnapshot.tabs[0];
  const activeDraft = screenshotSnapshot.drafts[0];

  const sidebar = (
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
  );

  const editorPane = (
    <div className="flex h-full min-h-0 flex-col bg-background">
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
        className="flex min-h-0 flex-1 flex-col gap-4 p-4"
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
        {isEditorResizableLayout ? (
          <ResizablePanelGroup
            className="min-h-0 flex-1 rounded-md border border-border bg-background"
            orientation="vertical"
          >
            <ResizablePanel className="overflow-hidden" defaultSize="58" minSize="280px">
              <div className="grid h-full min-h-0 gap-4 overflow-auto p-4 2xl:grid-cols-[minmax(0,1fr)_minmax(360px,0.9fr)]">
                <section className="flex shrink-0 flex-col gap-4">
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
            </ResizablePanel>
            <ResizableHandle
              aria-label="Resize request and response panels"
              orientation="vertical"
              withHandle
            />
            <ResizablePanel className="overflow-hidden" defaultSize="42" minSize="220px">
              <div className="grid h-full min-h-0 gap-4 overflow-auto p-4 xl:grid-cols-[minmax(260px,0.6fr)_minmax(0,1fr)]">
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
            </ResizablePanel>
          </ResizablePanelGroup>
        ) : (
        <div className="grid min-h-0 gap-4">
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
          <BodyEditor
            body={content.body}
            onChange={(body) => setContent((current) => ({ ...current, body }))}
            workspaceId={screenshotSnapshot.workspaceId}
          />
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
        )}
      </section>
    </div>
  );

  return (
    <main className="flex min-h-screen flex-col bg-muted text-foreground">
      <AppHeader
        checkingUpdates={false}
        density={density}
        diagnosticsOpen={false}
        locale={locale}
        newRequestPending={false}
        onCheckUpdates={() => undefined}
        onNewRequest={() => undefined}
        onRelinkBodyFiles={() => undefined}
        onSetBaseDirectory={() => undefined}
        onToggleDiagnostics={() => undefined}
        setDensity={setDensity}
        setLocale={setLocale}
        setTheme={setTheme}
        theme={theme}
        updateError={false}
        updateResult={null}
      />
      {isDesktopLayout ? (
        <ResizablePanelGroup className="min-h-0 flex-1" orientation="horizontal">
          <ResizablePanel
            className="overflow-hidden"
            defaultSize="24"
            maxSize="36"
            minSize="220px"
          >
            {sidebar}
          </ResizablePanel>
          <ResizableHandle aria-label="Resize collections and request workspace" withHandle />
          <ResizablePanel className="overflow-hidden" minSize="50">
            {editorPane}
          </ResizablePanel>
        </ResizablePanelGroup>
      ) : (
        <div className="flex min-h-0 flex-1 flex-col">
          {sidebar}
          {editorPane}
        </div>
      )}
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
  const isDesktopLayout = useDesktopLayout();
  const { locale, setLocale } = useI18n();
  const { density, setDensity, setTheme, theme } = usePreferences();
  useEffect(() => {
    setTheme(variant.theme);
    setDensity(variant.density);
  }, [setDensity, setTheme, variant]);

  const sidebar = (
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
  );
  const editorPane = (
    <div className="flex h-full min-h-0 flex-col bg-background">
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
        <Button type="button">New Request</Button>
      </section>
    </div>
  );

  return (
    <main className="flex min-h-screen flex-col bg-muted text-foreground">
      <AppHeader
        checkingUpdates={false}
        density={density}
        diagnosticsOpen={false}
        locale={locale}
        newRequestPending={false}
        onCheckUpdates={() => undefined}
        onNewRequest={() => undefined}
        onRelinkBodyFiles={() => undefined}
        onSetBaseDirectory={() => undefined}
        onToggleDiagnostics={() => undefined}
        setDensity={setDensity}
        setLocale={setLocale}
        setTheme={setTheme}
        theme={theme}
        updateError={false}
        updateResult={null}
      />
      {isDesktopLayout ? (
        <ResizablePanelGroup className="min-h-0 flex-1" orientation="horizontal">
          <ResizablePanel
            className="overflow-hidden"
            defaultSize="24"
            maxSize="36"
            minSize="220px"
          >
            {sidebar}
          </ResizablePanel>
          <ResizableHandle aria-label="Resize collections and request workspace" withHandle />
          <ResizablePanel className="overflow-hidden" minSize="50">
            {editorPane}
          </ResizablePanel>
        </ResizablePanelGroup>
      ) : (
        <div className="flex min-h-0 flex-1 flex-col">
          {sidebar}
          {editorPane}
        </div>
      )}
    </main>
  );
}

function useDesktopLayout() {
  return useMediaQuery("(min-width: 768px)", true);
}

function useMediaQuery(query: string, defaultMatches: boolean) {
  const [matches, setMatches] = useState(() =>
    typeof window === "undefined" ? defaultMatches : window.matchMedia(query).matches,
  );

  useEffect(() => {
    const media = window.matchMedia(query);
    setMatches(media.matches);
    const onChange = (event: MediaQueryListEvent) => setMatches(event.matches);
    media.addEventListener("change", onChange);
    return () => media.removeEventListener("change", onChange);
  }, [query]);

  return matches;
}
