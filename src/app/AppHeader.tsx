import { Bug, Folder, Plus, RefreshCw, RotateCcw } from "lucide-react";
import type { ReactNode } from "react";

import { Button } from "../components/ui/button";
import { NativeSelect } from "../components/ui/native-select";
import { Separator } from "../components/ui/separator";
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "../components/ui/tooltip";
import { useI18n, type AppLocale } from "./i18n";
import type { Density, Theme } from "./preferences";

type AppHeaderProps = {
  checkingUpdates: boolean;
  density: Density;
  diagnosticsOpen: boolean;
  locale: AppLocale;
  newRequestPending: boolean;
  onCheckUpdates: () => void;
  onNewRequest: () => void;
  onRelinkBodyFiles: () => void;
  onSetBaseDirectory: () => void;
  onToggleDiagnostics: () => void;
  setDensity: (density: Density) => void;
  setLocale: (locale: AppLocale) => void;
  setTheme: (theme: Theme) => void;
  theme: Theme;
  updateError: boolean;
  updateResult: { latestVersion: string; updateAvailable: boolean } | null;
};

export function AppHeader({
  checkingUpdates,
  density,
  diagnosticsOpen,
  locale,
  newRequestPending,
  onCheckUpdates,
  onNewRequest,
  onRelinkBodyFiles,
  onSetBaseDirectory,
  onToggleDiagnostics,
  setDensity,
  setLocale,
  setTheme,
  theme,
  updateError,
  updateResult,
}: AppHeaderProps) {
  const { t } = useI18n();

  return (
    <header className="relative flex min-h-12 flex-wrap items-center gap-2 border-b border-border bg-background px-3 py-2 sm:px-4">
      <h1 className="shrink-0 text-sm font-semibold">Postmite</h1>
      <div className="flex min-w-0 flex-1 flex-wrap items-center justify-end gap-2">
        <label className="sr-only" htmlFor="app-theme">{t("app.theme")}</label>
        <NativeSelect
          aria-label={t("app.theme")}
          className="w-[7.75rem] max-w-full"
          id="app-theme"
          onChange={(event) => setTheme(event.currentTarget.value as Theme)}
          value={theme}
        >
          <option value="light">{t("app.theme.light")}</option>
          <option value="dark">{t("app.theme.dark")}</option>
          <option value="system">{t("app.theme.system")}</option>
        </NativeSelect>
        <label className="sr-only" htmlFor="app-density">{t("app.density")}</label>
        <NativeSelect
          aria-label={t("app.density")}
          className="w-[8.5rem] max-w-full"
          id="app-density"
          onChange={(event) => setDensity(event.currentTarget.value as Density)}
          value={density}
        >
          <option value="comfortable">{t("app.density.comfortable")}</option>
          <option value="compact">{t("app.density.compact")}</option>
        </NativeSelect>
        <TooltipProvider delayDuration={0}>
          <HeaderTooltip label={checkingUpdates ? t("app.checkingUpdates") : t("app.checkUpdates")}>
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
              <span className="hidden xl:inline">
                {checkingUpdates ? t("app.checkingUpdates") : t("app.checkUpdates")}
              </span>
            </Button>
          </HeaderTooltip>
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
        <Separator className="hidden h-6 sm:block" orientation="vertical" />
        <label className="sr-only" htmlFor="app-language">{t("app.language")}</label>
        <NativeSelect
          aria-label={t("app.language")}
          className="w-[7.25rem] max-w-full"
          id="app-language"
          onChange={(event) => setLocale(event.currentTarget.value as AppLocale)}
          value={locale}
        >
          <option value="en">English</option>
          <option value="ja">日本語</option>
        </NativeSelect>
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
