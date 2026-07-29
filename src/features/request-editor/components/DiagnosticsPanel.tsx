import { Archive, Bug, X } from "lucide-react";
import { useEffect, useState } from "react";
import { useI18n } from "../../../app/i18n";

import {
  exportDiagnosticBundle,
  getDiagnosticBundlePreview,
  setDiagnosticDebugLogging,
} from "../../../shared/api/diagnostics";
import type { DiagnosticBundlePreviewDto } from "../../../shared/api/generated/ipc";

type DiagnosticsPanelProps = {
  onClose: () => void;
};

export function DiagnosticsPanel({ onClose }: DiagnosticsPanelProps) {
  const { t } = useI18n();
  const [preview, setPreview] = useState<DiagnosticBundlePreviewDto | null>(null);
  const [debugEnabled, setDebugEnabled] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    void getDiagnosticBundlePreview()
      .then((nextPreview) => {
        setPreview(nextPreview);
        setDebugEnabled(nextPreview.debugLoggingEnabled);
      })
      .catch(() => setError("Diagnostics are currently unavailable."));
  }, []);

  async function handleDebugChange(enabled: boolean) {
    setError(null);
    try {
      const status = await setDiagnosticDebugLogging({
        enabled,
        durationMinutes: enabled ? 15 : null,
      });
      setDebugEnabled(status.enabled);
    } catch {
      setError("Debug logging could not be updated.");
    }
  }

  async function handleExport() {
    const bundlePath = window.prompt("Diagnostic bundle path", "postmite-diagnostics.zip")?.trim();
    if (!bundlePath) {
      return;
    }
    setError(null);
    try {
      await exportDiagnosticBundle({ bundlePath });
    } catch {
      setError("Diagnostic bundle export failed.");
    }
  }

  return (
    <section
      aria-label={t("diagnostics.title")}
      className="absolute right-4 top-14 z-20 w-[min(28rem,calc(100vw-2rem))] border border-slate-300 bg-white p-4 shadow-lg"
    >
      <div className="flex items-center justify-between gap-3">
        <h2 className="text-sm font-semibold">{t("diagnostics.title")}</h2>
        <button aria-label={t("diagnostics.close")} className="p-1 hover:bg-slate-100" onClick={onClose} type="button">
          <X aria-hidden="true" size={16} />
        </button>
      </div>
      <label className="mt-4 flex items-center justify-between gap-3 text-sm">
        <span>Temporary debug logging</span>
        <input
          aria-label="Temporary debug logging"
          checked={debugEnabled}
          onChange={(event) => void handleDebugChange(event.currentTarget.checked)}
          type="checkbox"
        />
      </label>
      <p className="mt-1 text-xs text-slate-600">Debug logging expires after 15 minutes and never records request payloads or Secret values.</p>
      <h3 className="mt-4 text-sm font-medium">Bundle preview</h3>
      <ul className="mt-2 max-h-32 overflow-auto border border-slate-200 p-2 text-xs">
        {preview?.entries.map((entry) => <li key={entry}>{entry}</li>) ?? <li>Loading preview...</li>}
      </ul>
      <p className="mt-2 text-xs text-slate-600">Excluded: {preview?.exclusions.join("; ") ?? "loading"}</p>
      {error ? <p className="mt-3 text-sm text-red-700" role="alert">{error}</p> : null}
      <button
        className="mt-4 inline-flex h-8 items-center gap-2 border border-slate-700 bg-slate-900 px-3 text-sm font-medium text-white hover:bg-slate-700"
        disabled={!preview}
        onClick={() => void handleExport()}
        type="button"
      >
        <Archive aria-hidden="true" size={16} />
        Export bundle
      </button>
      <Bug aria-hidden="true" className="sr-only" />
    </section>
  );
}
