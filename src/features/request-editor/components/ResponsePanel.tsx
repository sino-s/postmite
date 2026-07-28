import type { ResponseExecutionState } from "../../../shared/api/execution";
import { formatBodyPreview, formatProxyMetadata } from "../request-editor-model";

type ResponsePanelProps = {
  execution: ResponseExecutionState | null;
};

export function ResponsePanel({ execution }: ResponsePanelProps) {
  if (!execution) {
    return (
      <section
        aria-label="Response"
        aria-live="polite"
        className="min-h-40 rounded-md border border-slate-300 bg-white p-3 text-sm text-slate-600"
      >
        No response yet.
      </section>
    );
  }

  const elapsedMs =
    (execution.completedAtMs ?? Date.now()) - execution.startedAtMs;
  const bodyPreview = formatBodyPreview(execution.bodyPreview);
  const phaseLabel = execution.phase[0].toUpperCase() + execution.phase.slice(1);

  return (
    <section
      aria-label="Response"
      aria-live="polite"
      className="grid min-h-40 gap-3 rounded-md border border-slate-300 bg-white p-3 text-sm"
    >
      <div className="flex flex-wrap items-center gap-3">
        <span className="rounded-md border border-slate-300 px-2 py-1 font-medium text-slate-700">
          {phaseLabel}
        </span>
        {execution.status ? (
          <span className="font-semibold text-slate-950">
            Status {execution.status}
          </span>
        ) : null}
        <span className="text-slate-600">Time {Math.max(0, elapsedMs)} ms</span>
        {execution.protocol ? (
          <span className="text-slate-600">{execution.protocol}</span>
        ) : null}
        <span className="text-slate-600">
          Timing {formatTiming(execution)}
        </span>
        {execution.downloadProgress ? (
          <span className="text-slate-600">
            Received {execution.downloadProgress.receivedBytes.toString()} bytes
          </span>
        ) : null}
        {execution.decodedBytes !== null ? (
          <span className="text-slate-600">
            Decoded {execution.decodedBytes.toString()} bytes
          </span>
        ) : null}
        {execution.wireBytes !== null ? (
          <span className="text-slate-600">
            Wire {execution.wireBytes.toString()} bytes
          </span>
        ) : null}
        {execution.uploadProgress ? (
          <span className="text-slate-600">
            Sent {execution.uploadProgress.sentBytes.toString()} bytes
          </span>
        ) : null}
        {execution.proxy ? (
          <span className="text-slate-600">
            Proxy {formatProxyMetadata(execution.proxy)}
          </span>
        ) : null}
        {execution.tlsVerification === false ? (
          <span className="rounded-md border border-amber-300 bg-amber-50 px-2 py-1 text-xs font-semibold text-amber-900">
            TLS verification off
          </span>
        ) : null}
      </div>

      {execution.error ? (
        <p className="rounded-md border border-red-200 bg-red-50 px-3 py-2 text-red-800">
          {execution.error}
        </p>
      ) : null}

      {execution.redirects.length > 0 ? (
        <div className="rounded-md border border-slate-200">
          <div className="border-b border-slate-200 bg-slate-50 px-2 py-2 text-xs font-semibold text-slate-600">
            Redirects
          </div>
          <div className="max-h-28 overflow-auto">
            {execution.redirects.map((redirect, index) => (
              <div
                className="grid gap-1 border-b border-slate-100 px-2 py-2 text-xs last:border-b-0"
                key={`${redirect.status}-${redirect.from}-${index}`}
              >
                <span className="font-semibold text-slate-700">
                  {redirect.status}
                </span>
                <span className="break-words text-slate-600">{redirect.from}</span>
                <span className="break-words text-slate-900">{redirect.to}</span>
              </div>
            ))}
          </div>
        </div>
      ) : null}

      <div className="grid min-h-0 gap-3 lg:grid-cols-[minmax(260px,0.45fr)_minmax(0,1fr)]">
        <div className="min-h-0 overflow-auto rounded-md border border-slate-200">
          <table className="w-full table-fixed border-collapse text-left text-xs">
            <thead>
              <tr className="border-b border-slate-200 bg-slate-50 text-slate-600">
                <th className="w-40 px-2 py-2 font-semibold">Header</th>
                <th className="px-2 py-2 font-semibold">Value</th>
              </tr>
            </thead>
            <tbody>
              {execution.headers.map((header, index) => (
                <tr className="border-b border-slate-100" key={`${header.name}-${index}`}>
                  <td className="break-words px-2 py-2 font-medium text-slate-700">
                    {header.name}
                  </td>
                  <td className="break-words px-2 py-2 text-slate-600">
                    {header.value}
                  </td>
                </tr>
              ))}
              {execution.headers.length === 0 ? (
                <tr>
                  <td className="px-2 py-5 text-center text-slate-500" colSpan={2}>
                    No response headers
                  </td>
                </tr>
              ) : null}
            </tbody>
          </table>
        </div>
        <div className="min-h-0 rounded-md border border-slate-200 bg-slate-950 p-3 text-slate-50">
          <pre className="max-h-52 overflow-auto whitespace-pre-wrap break-words text-xs leading-5">
            {bodyPreview || "No response body"}
          </pre>
          {execution.bodyTruncated ? (
            <p className="mt-2 text-xs text-amber-200">Response preview truncated.</p>
          ) : null}
        </div>
      </div>
    </section>
  );
}

function formatTiming(execution: ResponseExecutionState) {
  const timing = execution.timing;
  const timingParts: Array<[string, bigint | null]> = [
    ["queue", timing.queuedMs],
    ["dns", timing.dnsMs],
    ["connect", timing.connectMs],
    ["tls", timing.tlsMs],
    ["first byte", timing.firstByteMs],
    ["download", timing.downloadMs],
    ["total", timing.totalMs],
  ];
  const parts = timingParts
    .filter((entry): entry is [string, bigint] => entry[1] !== null)
    .map(([label, value]) => `${label} ${value.toString()} ms`);

  return parts.length > 0 ? parts.join(" / ") : "pending";
}
