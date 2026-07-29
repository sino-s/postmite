import { Ban, Play, Save } from "lucide-react";

import { Button } from "../../../components/ui/button";
import { Input } from "../../../components/ui/input";
import type { RequestContentDto } from "../../../shared/api/generated/ipc";
import type { ResponseExecutionPhase } from "../../../shared/api/execution";
import { queryFromUrl } from "../ordered-fields";
import { useI18n } from "../../../app/i18n";

const METHODS = ["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS"];

type RequestLineProps = {
  content: RequestContentDto;
  executionPhase: ResponseExecutionPhase;
  executionRunning: boolean;
  onCancel: () => void;
  onChange: (updater: (content: RequestContentDto) => RequestContentDto) => void;
  onExecute: () => void;
  onSave: () => void;
  saving: boolean;
};

function executionStatus(phase: ResponseExecutionPhase, t: ReturnType<typeof useI18n>["t"]) {
  const key = {
    idle: "app.executionIdle",
    running: "app.executionRunning",
    completed: "app.executionCompleted",
    failed: "app.executionFailed",
    cancelled: "app.executionCancelled",
  }[phase] as "app.executionIdle" | "app.executionRunning" | "app.executionCompleted" | "app.executionFailed" | "app.executionCancelled";
  return t(key);
}

export function RequestLine({
  content,
  executionPhase,
  executionRunning,
  onCancel,
  onChange,
  onExecute,
  onSave,
  saving,
}: RequestLineProps) {
  const { t } = useI18n();
  return (
    <div className="grid gap-3 rounded-md border border-slate-300 bg-white p-3 md:grid-cols-[180px_140px_minmax(0,1fr)_auto_auto_auto]">
      <p aria-atomic="true" aria-live="polite" className="sr-only" role="status">
        {executionStatus(executionPhase, t)}
      </p>
      <label className="sr-only" htmlFor="request-name">
        {t("request.name")}
      </label>
      <Input
        className="min-w-0"
        id="request-name"
        onChange={(event) =>
          onChange((current) => ({
            ...current,
            name: event.currentTarget.value,
          }))
        }
        placeholder={t("request.namePlaceholder")}
        value={content.name}
      />
      <label className="sr-only" htmlFor="request-method">
        {t("request.method")}
      </label>
      <select
        className="h-[var(--control-height)] rounded-md border border-input bg-background px-3 text-sm font-medium text-foreground focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-ring disabled:cursor-not-allowed disabled:opacity-50"
        id="request-method"
        onChange={(event) =>
          onChange((current) => ({
            ...current,
            method: event.currentTarget.value,
          }))
        }
        value={content.method}
      >
        {METHODS.map((method) => (
          <option key={method} value={method}>
            {method}
          </option>
        ))}
      </select>
      <label className="sr-only" htmlFor="request-url">
        {t("request.url")}
      </label>
      <Input
        className="min-w-0"
        id="request-url"
        onChange={(event) => {
          const url = event.currentTarget.value;
          onChange((current) => ({
            ...current,
            url,
            query: queryFromUrl(url),
          }));
        }}
        placeholder="https://example.test/resource?tag=one&tag="
        value={content.url}
      />
      <Button
        disabled={saving}
        onClick={onSave}
        type="button"
        variant="outline"
      >
        <Save aria-hidden="true" size={16} />
        {t("request.save")}
      </Button>
      <Button
        disabled={executionRunning}
        onClick={onExecute}
        type="button"
      >
        <Play aria-hidden="true" size={16} />
        {t("request.send")}
      </Button>
      <Button
        disabled={!executionRunning}
        onClick={onCancel}
        type="button"
        variant="destructive"
      >
        <Ban aria-hidden="true" size={16} />
        {t("request.cancel")}
      </Button>
    </div>
  );
}
