import { History, Pin, PinOff } from "lucide-react";

import type { ExecutionHistorySnapshotDto, ExecutionRecordDto } from "../../../shared/api/generated/ipc";
import { useI18n } from "../../../app/i18n";

type HistoryPanelProps = {
  history: ExecutionHistorySnapshotDto | null;
  loading: boolean;
  onOpen: (record: ExecutionRecordDto) => void;
  onToggleDisabled: (disabled: boolean) => void;
  onTogglePinned: (record: ExecutionRecordDto) => void;
};

export function HistoryPanel({
  history,
  loading,
  onOpen,
  onToggleDisabled,
  onTogglePinned,
}: HistoryPanelProps) {
  const { formatDate, formatNumber, t } = useI18n();
  const records = history?.records ?? [];

  return (
    <section
      aria-label={t("history.title")}
      className="grid min-h-40 gap-3 rounded-md border border-slate-300 bg-white p-3 text-sm"
    >
      <div className="flex items-center justify-between gap-3">
        <h2 className="inline-flex items-center gap-2 text-sm font-semibold">
          <History aria-hidden="true" size={16} />
          {t("history.title")}
        </h2>
        {loading ? <span className="text-xs text-slate-500">{t("history.loading")}</span> : null}
      </div>
      {history ? (
        <label className="inline-flex items-center gap-2 text-xs font-medium text-slate-700">
          <input
            checked={history.disabled}
            className="h-4 w-4 rounded border-slate-300 text-slate-900 focus:ring-sky-500"
            onChange={(event) => onToggleDisabled(event.currentTarget.checked)}
            type="checkbox"
          />
          Disable history
        </label>
      ) : null}
      <p className="rounded-md border border-amber-200 bg-amber-50 px-2 py-2 text-xs text-amber-900">
        {history?.warning ??
          "Unknown sensitive values inside arbitrary response bodies may not always be detected."}
      </p>
      <div className="max-h-72 overflow-auto rounded-md border border-slate-200">
        {records.map((record) => (
          <div
            className="grid gap-2 border-b border-slate-100 p-2 last:border-b-0"
            key={record.id}
          >
            <div className="flex items-start justify-between gap-2">
              <button
                className="min-w-0 text-left focus-visible:outline focus-visible:outline-2 focus-visible:outline-sky-500"
                onClick={() => onOpen(record)}
                type="button"
              >
                <span className="block truncate text-sm font-semibold text-slate-900">
                  {record.request.name}
                </span>
                <span className="block truncate text-xs text-slate-600">
                  {record.request.method} {record.request.url}
                </span>
              </button>
              <button
                aria-label={record.pinned ? "Unpin history record" : "Pin history record"}
                className="inline-flex h-8 w-8 shrink-0 items-center justify-center rounded-md text-slate-600 hover:bg-slate-100 focus-visible:outline focus-visible:outline-2 focus-visible:outline-sky-500"
                onClick={() => onTogglePinned(record)}
                title={record.pinned ? "Unpin history record" : "Pin history record"}
                type="button"
              >
                {record.pinned ? (
                  <PinOff aria-hidden="true" size={15} />
                ) : (
                  <Pin aria-hidden="true" size={15} />
                )}
              </button>
            </div>
            <div className="flex flex-wrap gap-2 text-xs text-slate-600">
              <span>{formatDate(Number(record.createdAtEpochSeconds) * 1000)}</span>
              {record.response.status ? <span>{t("response.status", { status: record.response.status })}</span> : null}
              {record.response.durationMs !== null ? (
                <span>{formatNumber(record.response.durationMs)} ms</span>
              ) : null}
              {record.response.error ? <span>{record.response.error}</span> : null}
            </div>
          </div>
        ))}
        {records.length === 0 ? (
          <p className="px-2 py-6 text-center text-sm text-slate-500">
            {t("history.title")}
          </p>
        ) : null}
      </div>
    </section>
  );
}
