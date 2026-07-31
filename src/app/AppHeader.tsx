import { Bug, Folder, Menu, Plus, RefreshCw, RotateCcw, Settings2 } from "lucide-react";
import { useState, type ReactNode } from "react";

import { Button } from "../components/ui/button";
import { NativeSelect } from "../components/ui/native-select";
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "../components/ui/tooltip";
import { useI18n, type AppLocale } from "./i18n";
import type { Density, RequestResponseSplit, Theme } from "./preferences";
import { SplitToggle } from "../features/request-editor/controls/SplitToggle";
import type { WorkspaceSummaryDto } from "../shared/api/generated/ipc";

type AppHeaderProps = {
  checkingUpdates: boolean;
  density: Density;
  diagnosticsOpen: boolean;
  locale: AppLocale;
  newRequestPending: boolean;
  onCheckUpdates: () => void;
  onNewRequest: () => void;
  onManageWorkspaces: () => void;
  onRelinkBodyFiles: () => void;
  onSetBaseDirectory: () => void;
  onToggleDiagnostics: () => void;
  onSelectWorkspace: (workspaceId: string) => void;
  requestResponseSplit: RequestResponseSplit;
  setDensity: (density: Density) => void;
  setLocale: (locale: AppLocale) => void;
  setRequestResponseSplit: (split: RequestResponseSplit) => void;
  setTheme: (theme: Theme) => void;
  theme: Theme;
  updateError: boolean;
  updateResult: { latestVersion: string; updateAvailable: boolean } | null;
  selectedWorkspaceId: string;
  workspaces: WorkspaceSummaryDto[];
};

export function AppHeader({
  checkingUpdates,
  density,
  diagnosticsOpen,
  locale,
  newRequestPending,
  onCheckUpdates,
  onNewRequest,
  onManageWorkspaces,
  onRelinkBodyFiles,
  onSetBaseDirectory,
  onToggleDiagnostics,
  onSelectWorkspace,
  requestResponseSplit,
  setDensity,
  setLocale,
  setRequestResponseSplit,
  setTheme,
  theme,
  updateError,
  updateResult,
  selectedWorkspaceId,
  workspaces,
}: AppHeaderProps) {
  const { t } = useI18n();
  const [menuOpen, setMenuOpen] = useState(false);

  return (
    <header className="relative flex min-h-12 flex-wrap items-center gap-2 border-b border-border bg-background px-3 py-2 sm:px-4">
      <div className="relative flex shrink-0 items-center gap-2">
        <Button
          aria-expanded={menuOpen}
          aria-label={t("app.menu")}
          aria-controls="app-header-menu"
          onClick={() => setMenuOpen((open) => !open)}
          size="icon"
          type="button"
          variant="outline"
        >
          <Menu aria-hidden="true" size={16} />
        </Button>
        <h1 className="text-sm font-semibold">Postmite</h1>
        {menuOpen ? (
          <div
            className="absolute left-0 top-full z-30 mt-2 grid w-72 gap-3 rounded-md border border-border bg-popover p-3 text-sm text-popover-foreground shadow-lg"
            id="app-header-menu"
          >
            <label className="grid gap-1 text-xs font-semibold" htmlFor="app-theme">
              {t("app.theme")}
              <NativeSelect
                aria-label={t("app.theme")}
                id="app-theme"
                onChange={(event) => setTheme(event.currentTarget.value as Theme)}
                value={theme}
              >
                <option value="light">{t("app.theme.light")}</option>
                <option value="dark">{t("app.theme.dark")}</option>
                <option value="system">{t("app.theme.system")}</option>
              </NativeSelect>
            </label>
            <label className="grid gap-1 text-xs font-semibold" htmlFor="app-density">
              {t("app.density")}
              <NativeSelect
                aria-label={t("app.density")}
                id="app-density"
                onChange={(event) => setDensity(event.currentTarget.value as Density)}
                value={density}
              >
                <option value="comfortable">{t("app.density.comfortable")}</option>
                <option value="compact">{t("app.density.compact")}</option>
              </NativeSelect>
            </label>
            <Button
              aria-label={checkingUpdates ? t("app.checkingUpdates") : t("app.checkUpdates")}
              aria-live="polite"
              disabled={checkingUpdates}
              onClick={onCheckUpdates}
              size="sm"
              type="button"
              variant="outline"
            >
              <RefreshCw aria-hidden="true" size={16} />
              {checkingUpdates ? t("app.checkingUpdates") : t("app.checkUpdates")}
            </Button>
            <label className="grid gap-1 text-xs font-semibold" htmlFor="app-language">
              {t("app.language")}
              <NativeSelect
                aria-label={t("app.language")}
                id="app-language"
                onChange={(event) => setLocale(event.currentTarget.value as AppLocale)}
                value={locale}
              >
                <option value="en">English</option>
                <option value="ja">日本語</option>
              </NativeSelect>
            </label>
          </div>
        ) : null}
      </div>
      <div className="flex min-w-0 flex-1 items-center gap-1 md:min-w-48 md:flex-none">
        <NativeSelect
          aria-label={t("workspace.current")}
          onChange={(event) => onSelectWorkspace(event.currentTarget.value)}
          value={selectedWorkspaceId}
        >
          {workspaces.map((workspace) => (
            <option key={workspace.id} value={workspace.id}>
              {workspace.name}
            </option>
          ))}
        </NativeSelect>
        <Button
          aria-label={t("workspace.manage")}
          onClick={onManageWorkspaces}
          size="icon"
          title={t("workspace.manage")}
          type="button"
          variant="outline"
        >
          <Settings2 aria-hidden="true" size={16} />
        </Button>
      </div>
      <div className="flex w-full min-w-0 flex-wrap items-center justify-end gap-2 md:w-auto md:flex-1">
        <TooltipProvider delayDuration={0}>
          <SplitToggle setSplit={setRequestResponseSplit} split={requestResponseSplit} />
          <Tooltip>
            <TooltipTrigger asChild>
              <Button
                aria-label={t("app.diagnostics")}
                data-state={diagnosticsOpen ? "open" : "closed"}
                onClick={onToggleDiagnostics}
                size="icon"
                type="button"
                variant="outline"
              >
                <Bug aria-hidden="true" size={16} />
              </Button>
            </TooltipTrigger>
            <TooltipContent>{t("app.diagnostics")}</TooltipContent>
          </Tooltip>
          <HeaderTooltip label={t("app.base")}>
            <Button
              aria-label={t("app.base")}
              onClick={onSetBaseDirectory}
              size="sm"
              type="button"
              variant="outline"
            >
              <Folder aria-hidden="true" size={16} />
              <span className="hidden md:inline">{t("app.base")}</span>
            </Button>
          </HeaderTooltip>
          <HeaderTooltip label={t("app.relink")}>
            <Button
              aria-label={t("app.relink")}
              onClick={onRelinkBodyFiles}
              size="sm"
              type="button"
              variant="outline"
            >
              <RotateCcw aria-hidden="true" size={16} />
              <span className="hidden lg:inline">{t("app.relink")}</span>
            </Button>
          </HeaderTooltip>
          <HeaderTooltip label={t("app.new")}>
            <Button
              aria-label={t("app.new")}
              disabled={newRequestPending}
              onClick={onNewRequest}
              size="sm"
              type="button"
            >
              <Plus aria-hidden="true" size={16} />
              <span className="hidden sm:inline">{t("app.new")}</span>
            </Button>
          </HeaderTooltip>
        </TooltipProvider>
      </div>
      {updateResult ? (
        <p
          className="absolute right-4 top-full z-20 mt-2 rounded-md border border-border bg-popover px-3 py-2 text-sm text-popover-foreground shadow-lg"
          role="status"
        >
          {updateResult.updateAvailable
            ? t("app.updateAvailable", { version: updateResult.latestVersion })
            : t("app.upToDate")}
        </p>
      ) : null}
      {updateError ? (
        <p
          className="absolute right-4 top-full z-20 mt-2 rounded-md border border-destructive bg-popover px-3 py-2 text-sm text-destructive shadow-lg"
          role="alert"
        >
          {t("app.updateCheckFailed")}
        </p>
      ) : null}
    </header>
  );
}

function HeaderTooltip({
  children,
  label,
}: {
  children: ReactNode;
  label: string;
}) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>{children}</TooltipTrigger>
      <TooltipContent>{label}</TooltipContent>
    </Tooltip>
  );
}
