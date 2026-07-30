import { useEffect, useMemo, useState } from "react";

import { AppHeader } from "./AppHeader";
import { Button } from "../components/ui/button";
import { I18nProvider, useI18n } from "./i18n";
import { PreferencesProvider, usePreferences } from "./preferences";
import type { RequestResponseSplit } from "./preferences";
import { CollectionsSidebar } from "../features/request-editor/panels/CollectionsSidebar";
import { RequestLine } from "../features/request-editor/controls/RequestLine";
import { TabStrip } from "../features/request-editor/controls/TabStrip";
import { screenshotCookies, screenshotExecution, screenshotHistory, screenshotResolution, screenshotSnapshot } from "../features/request-editor/fixtures/screenshot-fixtures";
import { useMediaQuery } from "../features/request-editor/hooks/useMediaQuery";
import { RequestEditorPanels } from "../features/request-editor/layout/RequestEditorPanels";
import { RequestWorkspaceShell } from "../features/request-editor/layout/RequestWorkspaceShell";
import type { RequestContentDto } from "../shared/api/generated/ipc";

type ScreenshotVariant = {
  density: "comfortable" | "compact";
  requestResponseSplit: RequestResponseSplit;
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
  const isEditorResizableLayout = useMediaQuery("(min-width: 1024px)", true);
  const variant = useMemo(readVariant, []);
  const { locale, setLocale } = useI18n();
  const {
    density,
    requestResponseSplit,
    setDensity,
    setRequestResponseSplit,
    setTheme,
    theme,
  } = usePreferences();
  const [content, setContent] = useState<RequestContentDto>(
    screenshotSnapshot.drafts[0].content,
  );

  useEffect(() => {
    setTheme(variant.theme);
    setDensity(variant.density);
    setRequestResponseSplit(variant.requestResponseSplit);
  }, [setDensity, setRequestResponseSplit, setTheme, variant]);

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
          executionReady
          executionRunning={false}
          onCancel={() => undefined}
          onChange={(updater) => setContent((current) => updater(current))}
          onExecute={() => undefined}
          onSave={() => undefined}
          saving={false}
        />
        <RequestEditorPanels
          content={content}
          cookies={screenshotCookies}
          cookiesLoading={false}
          execution={screenshotExecution}
          history={screenshotHistory}
          historyLoading={false}
          onChange={(updater) => setContent((current) => updater(current))}
          onClearCookies={() => undefined}
          onDeleteCookie={() => undefined}
          onOpenHistoryRecord={() => undefined}
          onRevealCookie={async () => ({ value: "" })}
          onSaveCookie={() => undefined}
          onToggleHistoryDisabled={() => undefined}
          onToggleHistoryPinned={() => undefined}
          requestResponseSplit={requestResponseSplit}
          resizable={isEditorResizableLayout}
          resolution={screenshotResolution}
          resolving={false}
          setRequestResponseSplit={setRequestResponseSplit}
          workspaceId={screenshotSnapshot.workspaceId}
        />
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
      <RequestWorkspaceShell editorPane={editorPane} sidebar={sidebar} />
    </main>
  );
}

function readVariant(): ScreenshotVariant {
  const params = new URLSearchParams(window.location.search);
  return {
    density: params.get("density") === "compact" ? "compact" : "comfortable",
    requestResponseSplit: params.get("split") === "vertical" ? "vertical" : "horizontal",
    state: params.get("state") === "empty" ? "empty" : "workspace",
    theme: params.get("theme") === "dark" ? "dark" : "light",
  };
}

function EmptyWorkspace({ variant }: { variant: ScreenshotVariant }) {
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
      <RequestWorkspaceShell editorPane={editorPane} sidebar={sidebar} />
    </main>
  );
}
