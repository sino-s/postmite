import { useState } from "react";
import { Cookie, Edit3, FileText, RotateCcw, Save, Trash2 } from "lucide-react";

import type { WorkspaceCookieDto, CookieSameSiteDto } from "../../../shared/api/generated/ipc";
import { emptyCookieForm, formatSameSite, type CookieFormValue } from "../request-editor-model";
import { IconButton } from "./IconButton";

type CookiePanelProps = {
  cookies: WorkspaceCookieDto[];
  loading: boolean;
  onClear: () => void;
  onDelete: (cookie: WorkspaceCookieDto) => void;
  onReveal: (cookie: WorkspaceCookieDto) => Promise<{ value: string }>;
  onSave: (input: CookieFormValue) => void;
};

export function CookiePanel({
  cookies,
  loading,
  onClear,
  onDelete,
  onReveal,
  onSave,
}: CookiePanelProps) {
  const [draft, setDraft] = useState<CookieFormValue>(emptyCookieForm());
  const [revealed, setRevealed] = useState<Record<string, string>>({});

  function editCookie(cookie: WorkspaceCookieDto) {
    setDraft({
      cookieId: cookie.id,
      name: cookie.name,
      value: "",
      domain: cookie.domain,
      path: cookie.path,
      secure: cookie.secure,
      httpOnly: cookie.httpOnly,
      sameSite: cookie.sameSite,
      expiresAtEpochSeconds: cookie.expiresAtEpochSeconds,
    });
  }

  async function reveal(cookie: WorkspaceCookieDto) {
    if (
      !window.confirm(
        `Reveal the ${cookie.name} cookie value? This may expose a Secret on screen.`,
      )
    ) {
      return;
    }
    const value = await onReveal(cookie);
    setRevealed((current) => ({
      ...current,
      [cookie.id]: value.value,
    }));
  }

  function handleSubmit(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    onSave(draft);
    setDraft(emptyCookieForm());
  }

  return (
    <section
      aria-label="Cookie jar"
      className="grid min-h-40 gap-3 rounded-md border border-slate-300 bg-white p-3 text-sm"
    >
      <div className="flex items-center justify-between gap-3">
        <h2 className="inline-flex items-center gap-2 text-sm font-semibold">
          <Cookie aria-hidden="true" size={16} />
          Cookies
        </h2>
        {loading ? <span className="text-xs text-slate-500">Loading</span> : null}
      </div>
      <form className="grid gap-2" onSubmit={handleSubmit}>
        <div className="grid gap-2 sm:grid-cols-2">
          <label className="grid gap-1 text-xs font-medium text-slate-700">
            Cookie name
            <input
              className="h-8 rounded-md border border-slate-300 px-2 text-sm focus:border-sky-500 focus:outline focus:outline-2 focus:outline-sky-500"
              onChange={(event) => {
                const name = event.currentTarget.value;
                setDraft((current) => ({ ...current, name }));
              }}
              required
              value={draft.name}
            />
          </label>
          <label className="grid gap-1 text-xs font-medium text-slate-700">
            Cookie value
            <input
              className="h-8 rounded-md border border-slate-300 px-2 text-sm focus:border-sky-500 focus:outline focus:outline-2 focus:outline-sky-500"
              onChange={(event) => {
                const value = event.currentTarget.value;
                setDraft((current) => ({ ...current, value }));
              }}
              required
              type="password"
              value={draft.value}
            />
          </label>
          <label className="grid gap-1 text-xs font-medium text-slate-700">
            Cookie domain
            <input
              className="h-8 rounded-md border border-slate-300 px-2 text-sm focus:border-sky-500 focus:outline focus:outline-2 focus:outline-sky-500"
              onChange={(event) => {
                const domain = event.currentTarget.value;
                setDraft((current) => ({ ...current, domain }));
              }}
              required
              value={draft.domain}
            />
          </label>
          <label className="grid gap-1 text-xs font-medium text-slate-700">
            Cookie path
            <input
              className="h-8 rounded-md border border-slate-300 px-2 text-sm focus:border-sky-500 focus:outline focus:outline-2 focus:outline-sky-500"
              onChange={(event) => {
                const path = event.currentTarget.value;
                setDraft((current) => ({ ...current, path }));
              }}
              required
              value={draft.path}
            />
          </label>
        </div>
        <div className="grid gap-2 sm:grid-cols-[1fr_auto_auto]">
          <label className="grid gap-1 text-xs font-medium text-slate-700">
            SameSite
            <select
              className="h-8 rounded-md border border-slate-300 bg-white px-2 text-sm focus:border-sky-500 focus:outline focus:outline-2 focus:outline-sky-500"
              onChange={(event) => {
                const sameSite = (event.currentTarget.value || null) as
                  | CookieSameSiteDto
                  | null;
                setDraft((current) => ({
                  ...current,
                  sameSite,
                }));
              }}
              value={draft.sameSite ?? ""}
            >
              <option value="">Unset</option>
              <option value="STRICT">Strict</option>
              <option value="LAX">Lax</option>
              <option value="NONE">None</option>
            </select>
          </label>
          <label className="inline-flex items-end gap-2 pb-1 text-xs font-medium text-slate-700">
            <input
              checked={draft.secure}
              className="h-4 w-4 rounded border-slate-300 text-slate-900 focus:ring-sky-500"
              onChange={(event) => {
                const secure = event.currentTarget.checked;
                setDraft((current) => ({ ...current, secure }));
              }}
              type="checkbox"
            />
            Secure
          </label>
          <label className="inline-flex items-end gap-2 pb-1 text-xs font-medium text-slate-700">
            <input
              checked={draft.httpOnly}
              className="h-4 w-4 rounded border-slate-300 text-slate-900 focus:ring-sky-500"
              onChange={(event) => {
                const httpOnly = event.currentTarget.checked;
                setDraft((current) => ({ ...current, httpOnly }));
              }}
              type="checkbox"
            />
            HttpOnly
          </label>
        </div>
        <div className="flex flex-wrap gap-2">
          <button
            className="inline-flex h-8 items-center gap-2 rounded-md bg-slate-900 px-3 text-xs font-medium text-white hover:bg-slate-700 focus-visible:outline focus-visible:outline-2 focus-visible:outline-sky-500"
            type="submit"
          >
            <Save aria-hidden="true" size={14} />
            {draft.cookieId ? "Update cookie" : "Add cookie"}
          </button>
          <button
            className="inline-flex h-8 items-center gap-2 rounded-md border border-slate-300 px-3 text-xs font-medium hover:bg-slate-50 focus-visible:outline focus-visible:outline-2 focus-visible:outline-sky-500"
            onClick={() => setDraft(emptyCookieForm())}
            type="button"
          >
            <RotateCcw aria-hidden="true" size={14} />
            Reset
          </button>
          <button
            className="inline-flex h-8 items-center gap-2 rounded-md border border-red-300 px-3 text-xs font-medium text-red-700 hover:bg-red-50 focus-visible:outline focus-visible:outline-2 focus-visible:outline-sky-500"
            onClick={onClear}
            type="button"
          >
            <Trash2 aria-hidden="true" size={14} />
            Clear cookies
          </button>
        </div>
      </form>
      <div className="max-h-72 overflow-auto rounded-md border border-slate-200">
        {cookies.map((cookie) => (
          <div className="grid gap-2 border-b border-slate-100 p-2 last:border-b-0" key={cookie.id}>
            <div className="flex items-start justify-between gap-2">
              <div className="min-w-0">
                <p className="truncate text-sm font-semibold text-slate-900">{cookie.name}</p>
                <p className="truncate text-xs text-slate-600">
                  {cookie.domain}{cookie.path} {cookie.secure ? "Secure" : "Insecure"}
                </p>
              </div>
              <div className="flex shrink-0 items-center">
                <IconButton label={`Inspect ${cookie.name} cookie`} onClick={() => void reveal(cookie)}>
                  <FileText aria-hidden="true" size={14} />
                </IconButton>
                <IconButton label={`Edit ${cookie.name} cookie`} onClick={() => editCookie(cookie)}>
                  <Edit3 aria-hidden="true" size={14} />
                </IconButton>
                <IconButton label={`Delete ${cookie.name} cookie`} onClick={() => onDelete(cookie)}>
                  <Trash2 aria-hidden="true" size={14} />
                </IconButton>
              </div>
            </div>
            <div className="grid gap-1 text-xs text-slate-600">
              <span>Value {revealed[cookie.id] ?? cookie.valuePreview}</span>
              <span>
                {cookie.session ? "Session" : "Persistent"}
                {cookie.sameSite ? ` SameSite ${formatSameSite(cookie.sameSite)}` : ""}
                {!cookie.hasValue ? " value unavailable until edited" : ""}
              </span>
            </div>
          </div>
        ))}
        {cookies.length === 0 ? (
          <p className="px-2 py-6 text-center text-sm text-slate-500">No cookies</p>
        ) : null}
      </div>
    </section>
  );
}
