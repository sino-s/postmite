import { Ban, Play, Save } from "lucide-react";

import type { RequestContentDto } from "../../../shared/api/generated/ipc";
import { queryFromUrl } from "../ordered-fields";
import { useI18n } from "../../../app/i18n";

const METHODS = ["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS"];

type RequestLineProps = {
  content: RequestContentDto;
  executionRunning: boolean;
  onCancel: () => void;
  onChange: (updater: (content: RequestContentDto) => RequestContentDto) => void;
  onExecute: () => void;
  onSave: () => void;
  saving: boolean;
};

export function RequestLine({
  content,
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
      <label className="sr-only" htmlFor="request-name">
        {t("request.name")}
      </label>
      <input
        className="h-10 min-w-0 rounded-md border border-slate-300 px-3 text-sm focus:border-sky-500 focus:outline focus:outline-2 focus:outline-sky-500"
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
        className="h-10 rounded-md border border-slate-300 bg-white px-3 text-sm font-medium focus:border-sky-500 focus:outline focus:outline-2 focus:outline-sky-500"
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
      <input
        className="h-10 min-w-0 rounded-md border border-slate-300 px-3 text-sm focus:border-sky-500 focus:outline focus:outline-2 focus:outline-sky-500"
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
      <button
        className="inline-flex h-10 items-center justify-center gap-2 rounded-md border border-slate-300 bg-white px-3 text-sm font-medium hover:bg-slate-50 focus-visible:outline focus-visible:outline-2 focus-visible:outline-sky-500"
        disabled={saving}
        onClick={onSave}
        type="button"
      >
        <Save aria-hidden="true" size={16} />
        {t("request.save")}
      </button>
      <button
        className="inline-flex h-10 items-center justify-center gap-2 rounded-md bg-sky-700 px-3 text-sm font-semibold text-white hover:bg-sky-600 focus-visible:outline focus-visible:outline-2 focus-visible:outline-sky-500"
        disabled={executionRunning}
        onClick={onExecute}
        type="button"
      >
        <Play aria-hidden="true" size={16} />
        {t("request.send")}
      </button>
      <button
        className="inline-flex h-10 items-center justify-center gap-2 rounded-md border border-red-300 bg-white px-3 text-sm font-medium text-red-700 hover:bg-red-50 focus-visible:outline focus-visible:outline-2 focus-visible:outline-sky-500 disabled:cursor-not-allowed disabled:opacity-60"
        disabled={!executionRunning}
        onClick={onCancel}
        type="button"
      >
        <Ban aria-hidden="true" size={16} />
        {t("request.cancel")}
      </button>
    </div>
  );
}
