import { History, Pin, PinOff } from "lucide-react";

import { Button } from "../../../components/ui/button";
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
      className="grid min-h-40 min-w-0 gap-3 rounded-md border border-border bg-card p-3 text-sm text-card-foreground"
    >
      <div className="flex items-center justify-between gap-3">
        <h2 className="inline-flex items-center gap-2 text-sm font-semibold">
          <History aria-hidden="true" size={16} />
          {t("history.title")}
        </h2>
        {loading ? <span className="text-xs text-muted-foreground">{t("history.loading")}</span> : null}
      </div>
      {history ? (
        <label className="inline-flex items-center gap-2 text-xs font-medium text-muted-foreground">
          <input
            checked={history.disabled}
            className="h-4 w-4 rounded border-input bg-background text-primary focus:ring-ring"
            onChange={(event) => onToggleDisabled(event.currentTarget.checked)}
            type="checkbox"
          />
          Disable history
        </label>
      ) : null}
      <p className="rounded-md border border-amber-300 bg-amber-50 px-2 py-2 text-xs text-amber-950 dark:bg-amber-950 dark:text-amber-100">
        {history?.warning ??
          "Unknown sensitive values inside arbitrary response bodies may not always be detected."}
      </p>
      <div className="max-h-72 min-w-0 overflow-auto rounded-md border border-border">
        {records.map((record) => (
          <div
            className="grid min-w-0 gap-2 border-b border-border p-2 last:border-b-0"
            key={record.id}
          >
            <div className="flex min-w-0 items-start justify-between gap-2">
              <Button
                className="h-auto min-w-0 flex-1 justify-start p-0 text-left hover:bg-transparent hover:text-foreground"
                onClick={() => onOpen(record)}
                type="button"
                variant="ghost"
              >
                <span className="min-w-0">
                <span className="block truncate text-sm font-semibold text-foreground">
                  {record.request.name}
                </span>
                <span className="block truncate text-xs text-muted-foreground">
                  {record.request.method} {record.request.url}
                </span>
                </span>
              </Button>
              <Button
                aria-label={record.pinned ? "Unpin history record" : "Pin history record"}
                className="text-muted-foreground hover:text-foreground"
                onClick={() => onTogglePinned(record)}
                size="icon"
                title={record.pinned ? "Unpin history record" : "Pin history record"}
                type="button"
                variant="ghost"
              >
                {record.pinned ? (
                  <PinOff aria-hidden="true" size={15} />
                ) : (
                  <Pin aria-hidden="true" size={15} />
                )}
              </Button>
            </div>
            <div className="flex min-w-0 flex-wrap gap-2 text-xs text-muted-foreground">
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
          <p className="px-2 py-6 text-center text-sm text-muted-foreground">
            {t("history.title")}
          </p>
        ) : null}
      </div>
    </section>
  );
}
