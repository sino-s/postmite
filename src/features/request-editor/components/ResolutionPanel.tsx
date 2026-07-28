import type { ResolvedRequestContentDto } from "../../../shared/api/generated/ipc";
import { formatResolutionError, formatVariableSource, sortResolvedFields } from "../request-editor-model";

type ResolutionPanelProps = {
  resolution: ResolvedRequestContentDto | null;
  resolving: boolean;
};

export function ResolutionPanel({ resolution, resolving }: ResolutionPanelProps) {
  const references = resolution?.references ?? [];
  const errors = resolution?.errors ?? [];
  const headers = resolution?.headers ?? [];
  const query = resolution?.query ?? [];

  return (
    <section
      aria-label="Variable resolution"
      className="min-h-40 rounded-md border border-slate-300 bg-white p-3 text-sm"
    >
      <div className="mb-3 flex items-center justify-between gap-3">
        <h2 className="text-sm font-semibold">Variables</h2>
        {resolving ? <span className="text-xs text-slate-500">Resolving</span> : null}
      </div>
      {errors.length > 0 ? (
        <div className="mb-3 rounded-md border border-red-200 bg-red-50 p-2 text-xs text-red-800">
          {errors.map((error) => (
            <p key={`${error.name}-${error.kind}`}>
              {error.name}: {formatResolutionError(error.kind)}
            </p>
          ))}
        </div>
      ) : null}
      {resolution?.unsafeTlsVisible ? (
        <p className="mb-3 rounded-md border border-amber-300 bg-amber-50 px-2 py-2 text-xs font-semibold text-amber-900">
          TLS verification is disabled for this request.
        </p>
      ) : null}
      <div className="overflow-x-auto rounded-md border border-slate-200">
        <table className="w-full min-w-[360px] table-fixed border-collapse text-left text-xs">
          <thead>
            <tr className="border-b border-slate-200 bg-slate-50 text-slate-600">
              <th className="w-32 px-2 py-2 font-semibold">Name</th>
              <th className="w-28 px-2 py-2 font-semibold">Source</th>
              <th className="px-2 py-2 font-semibold">Value</th>
            </tr>
          </thead>
          <tbody>
            {references.map((reference) => (
              <tr className="border-b border-slate-100" key={reference.name}>
                <td className="break-words px-2 py-2 font-medium text-slate-700">
                  {reference.name}
                </td>
                <td className="px-2 py-2 text-slate-600">
                  {formatVariableSource(reference.source)}
                </td>
                <td className="break-words px-2 py-2 text-slate-600">
                  {reference.value.value}
                </td>
              </tr>
            ))}
            {references.length === 0 ? (
              <tr>
                <td className="px-2 py-5 text-center text-slate-500" colSpan={3}>
                  No variables resolved
                </td>
              </tr>
            ) : null}
          </tbody>
        </table>
      </div>
      <div className="mt-3 grid gap-3 lg:grid-cols-2">
        <ResolvedFieldPreview fields={query} title="Final Params" />
        <ResolvedFieldPreview fields={headers} title="Final Headers" />
      </div>
    </section>
  );
}

function ResolvedFieldPreview({
  fields,
  title,
}: {
  fields: ResolvedRequestContentDto["headers"];
  title: string;
}) {
  const enabledFields = sortResolvedFields(fields).filter((field) => field.enabled);
  return (
    <div className="overflow-x-auto rounded-md border border-slate-200">
      <div className="border-b border-slate-200 bg-slate-50 px-2 py-2 text-xs font-semibold text-slate-600">
        {title}
      </div>
      <table className="w-full min-w-[260px] table-fixed border-collapse text-left text-xs">
        <tbody>
          {enabledFields.map((field, index) => (
            <tr className="border-b border-slate-100" key={`${field.order}-${index}`}>
              <td className="w-32 break-words px-2 py-2 font-medium text-slate-700">
                {field.name.value}
              </td>
              <td className="break-words px-2 py-2 text-slate-600">
                {field.value.value}
              </td>
            </tr>
          ))}
          {enabledFields.length === 0 ? (
            <tr>
              <td className="px-2 py-4 text-center text-slate-500" colSpan={2}>
                None
              </td>
            </tr>
          ) : null}
        </tbody>
      </table>
    </div>
  );
}
